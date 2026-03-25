use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::{
    div, hsla, px, size, point, AppContext, Bounds, Context, Entity, Global,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions,
};

/// App-lifetime handle that keeps the OverlayController entity alive via GPUI's global storage.
#[allow(dead_code)]
pub struct OverlayHandle(pub Entity<OverlayController>);
impl Global for OverlayHandle {}

use crate::app::{OverlayPhase, SharedState};
use crate::config::{OverlayConfig, OverlayPosition, OverlayStyle};

/// Fallback notch width if detection fails.
const NOTCH_WIDTH_FALLBACK: u32 = 180;
/// Visible height of the notch overlay content.
const NOTCH_HEIGHT: f64 = 60.0;
/// Bottom corner radius for the notch panel.
const NOTCH_CORNER_RADIUS: f64 = 10.0;
/// Number of EQ bars in the notch overlay.
const NOTCH_BAR_COUNT: usize = 16;
const NOTCH_BAR_GAP: f64 = 2.0;
const NOTCH_BAR_WIDTH: f64 = 4.0;
const NOTCH_DOT_COUNT: usize = 3;
const NOTCH_DOT_GAP: f64 = 8.0;
const NOTCH_DOT_SIZE: f64 = 6.0;
const NOTCH_BAR_TOP_INSET: f64 = 6.0;
const NOTCH_BAR_MAX_HEIGHT: f64 = 40.0;
const NOTCH_DOT_BOTTOM_INSET: f64 = 12.0;

// ---------------------------------------------------------------------------
// Native NSPanel FFI for notch mode
// ---------------------------------------------------------------------------

#[link(name = "AppKit", kind = "framework")]
#[link(name = "QuartzCore", kind = "framework")]
unsafe extern "C" {}

unsafe extern "C" {
    fn objc_getClass(name: *const u8) -> *mut c_void;
    fn sel_registerName(name: *const u8) -> *mut c_void;
    fn objc_msgSend(receiver: *mut c_void, sel: *mut c_void) -> *mut c_void;
}

// Typed function pointers for objc_msgSend with extra arguments (ARM64 ABI)
type MsgSendF64 = unsafe extern "C" fn(*mut c_void, *mut c_void, f64) -> *mut c_void;
type MsgSendF32 = unsafe extern "C" fn(*mut c_void, *mut c_void, f32) -> *mut c_void;
type MsgSendBool = unsafe extern "C" fn(*mut c_void, *mut c_void, bool) -> *mut c_void;
type MsgSendI64 = unsafe extern "C" fn(*mut c_void, *mut c_void, i64) -> *mut c_void;
type MsgSendU64 = unsafe extern "C" fn(*mut c_void, *mut c_void, u64) -> *mut c_void;
type MsgSendPtr = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
type MsgSendRect = unsafe extern "C" fn(*mut c_void, *mut c_void) -> NSRect;
type MsgSendRectBoolBool = unsafe extern "C" fn(
    *mut c_void, *mut c_void,
    NSRect, // contentRect
    u64,    // styleMask
    u64,    // backing
    bool,   // defer
) -> *mut c_void;
type MsgSendSetRect = unsafe extern "C" fn(*mut c_void, *mut c_void, NSRect, bool);

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct NSRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Shared state for the native notch panel, updated from the polling timer.
struct NotchPanelState {
    panel: *mut c_void,
    bar_layers: Vec<*mut c_void>,
    dot_layers: Vec<*mut c_void>,
    #[allow(dead_code)]
    content_view: *mut c_void,
    width: f64,
    height: f64,
    eq_bars: Vec<f32>,
    loading_tick: usize,
}

unsafe impl Send for NotchPanelState {}
unsafe impl Sync for NotchPanelState {}

/// Create a native borderless NSPanel for the notch overlay.
fn create_notch_panel(_shared: &SharedState) -> Option<Arc<Mutex<NotchPanelState>>> {
    let notch_w = crate::config::notch_width().unwrap_or(NOTCH_WIDTH_FALLBACK) as f64;
    let notch_h = NOTCH_HEIGHT;
    let bar_count = NOTCH_BAR_COUNT;

    unsafe {
        let msg_ptr: MsgSendPtr = std::mem::transmute(objc_msgSend as *const ());
        let msg_bool: MsgSendBool = std::mem::transmute(objc_msgSend as *const ());
        let msg_i64: MsgSendI64 = std::mem::transmute(objc_msgSend as *const ());
        let msg_u64: MsgSendU64 = std::mem::transmute(objc_msgSend as *const ());
        let msg_f64: MsgSendF64 = std::mem::transmute(objc_msgSend as *const ());
        let msg_rect: MsgSendRect = std::mem::transmute(objc_msgSend as *const ());
        let msg_init_rect: MsgSendRectBoolBool = std::mem::transmute(objc_msgSend as *const ());
        let msg_set_rect: MsgSendSetRect = std::mem::transmute(objc_msgSend as *const ());

        // Get screen frame
        let ns_screen = objc_getClass(b"NSScreen\0".as_ptr());
        let main_screen = objc_msgSend(ns_screen, sel_registerName(b"mainScreen\0".as_ptr()));
        if main_screen.is_null() { return None; }
        let screen_frame = msg_rect(main_screen, sel_registerName(b"frame\0".as_ptr()));

        // Calculate position: centered horizontally, top of screen
        let x = (screen_frame.w - notch_w) / 2.0;
        // In macOS coordinates (origin bottom-left), top of screen = maxY - height
        let y_final = screen_frame.y + screen_frame.h - notch_h;
        // Start hidden above the screen for slide animation
        let y_hidden = screen_frame.y + screen_frame.h;

        // Create NSPanel with borderless + nonactivating style
        let ns_panel_class = objc_getClass(b"NSPanel\0".as_ptr());
        let panel = objc_msgSend(ns_panel_class, sel_registerName(b"alloc\0".as_ptr()));
        let content_rect = NSRect { x, y: y_hidden, w: notch_w, h: notch_h };
        // styleMask: borderless (0) | nonactivatingPanel (1 << 7 = 128)
        let panel = msg_init_rect(
            panel,
            sel_registerName(b"initWithContentRect:styleMask:backing:defer:\0".as_ptr()),
            content_rect,
            128, // NSNonactivatingPanel (borderless is 0)
            2,   // NSBackingStoreBuffered
            false,
        );
        if panel.is_null() { return None; }

        // Configure panel
        let clear_color = objc_msgSend(
            objc_getClass(b"NSColor\0".as_ptr()),
            sel_registerName(b"clearColor\0".as_ptr()),
        );
        msg_ptr(panel, sel_registerName(b"setBackgroundColor:\0".as_ptr()), clear_color);
        msg_bool(panel, sel_registerName(b"setOpaque:\0".as_ptr()), false);
        msg_bool(panel, sel_registerName(b"setHasShadow:\0".as_ptr()), false);
        // Level: screenSaver (1000) to be above everything
        msg_i64(panel, sel_registerName(b"setLevel:\0".as_ptr()), 1000);
        msg_bool(panel, sel_registerName(b"setIgnoresMouseEvents:\0".as_ptr()), true);
        // Join all spaces
        msg_u64(panel, sel_registerName(b"setCollectionBehavior:\0".as_ptr()), 1 << 0); // canJoinAllSpaces
        msg_bool(panel, sel_registerName(b"setHidesOnDeactivate:\0".as_ptr()), false);

        // Create content view
        let ns_view_class = objc_getClass(b"NSView\0".as_ptr());
        let content_view = objc_msgSend(ns_view_class, sel_registerName(b"alloc\0".as_ptr()));
        let content_view = objc_msgSend(content_view, sel_registerName(b"init\0".as_ptr()));
        let view_rect = NSRect { x: 0.0, y: 0.0, w: notch_w, h: notch_h };
        msg_set_rect(content_view, sel_registerName(b"setFrame:\0".as_ptr()), view_rect, false);

        // Enable layer backing
        msg_bool(content_view, sel_registerName(b"setWantsLayer:\0".as_ptr()), true);
        let layer = objc_msgSend(content_view, sel_registerName(b"layer\0".as_ptr()));

        // Set black background on the layer
        let cg_black = objc_msgSend(
            objc_getClass(b"NSColor\0".as_ptr()),
            sel_registerName(b"blackColor\0".as_ptr()),
        );
        let cg_color = objc_msgSend(cg_black, sel_registerName(b"CGColor\0".as_ptr()));
        msg_ptr(layer, sel_registerName(b"setBackgroundColor:\0".as_ptr()), cg_color);

        // Round only bottom corners: maskedCorners = kCALayerMinXMinYCorner | kCALayerMaxXMinYCorner
        // In macOS coordinates (origin bottom-left), "min Y" is bottom
        msg_f64(layer, sel_registerName(b"setCornerRadius:\0".as_ptr()), NOTCH_CORNER_RADIUS);
        msg_u64(layer, sel_registerName(b"setMaskedCorners:\0".as_ptr()), 1 | 2); // bottom-left | bottom-right

        // Set as panel's content view
        msg_ptr(panel, sel_registerName(b"setContentView:\0".as_ptr()), content_view);

        // Create bar sublayers
        let total_bars_width = bar_count as f64 * NOTCH_BAR_WIDTH
            + (bar_count as f64 - 1.0) * NOTCH_BAR_GAP;
        let start_x = (notch_w - total_bars_width) / 2.0;

        let ca_layer_class = objc_getClass(b"CALayer\0".as_ptr());
        let white_color = {
            type MsgSendF64F64 = unsafe extern "C" fn(*mut c_void, *mut c_void, f64, f64) -> *mut c_void;
            let msg_f64_f64: MsgSendF64F64 = std::mem::transmute(objc_msgSend as *const ());
            let ns_color = msg_f64_f64(
                objc_getClass(b"NSColor\0".as_ptr()),
                sel_registerName(b"colorWithWhite:alpha:\0".as_ptr()),
                0.78,
                0.9,
            );
            objc_msgSend(ns_color, sel_registerName(b"CGColor\0".as_ptr()))
        };

        let mut bar_layers = Vec::with_capacity(bar_count);
        for i in 0..bar_count {
            let bar_layer = objc_msgSend(ca_layer_class, sel_registerName(b"new\0".as_ptr()));
            let bx = start_x + i as f64 * (NOTCH_BAR_WIDTH + NOTCH_BAR_GAP);
            let bar_rect = NSRect {
                x: bx,
                y: notch_h - NOTCH_BAR_TOP_INSET - 2.0,
                w: NOTCH_BAR_WIDTH,
                h: 2.0,
            };

            type MsgSendSetCGRect = unsafe extern "C" fn(*mut c_void, *mut c_void, NSRect);
            let msg_set_cg_rect: MsgSendSetCGRect = std::mem::transmute(objc_msgSend as *const ());
            msg_set_cg_rect(bar_layer, sel_registerName(b"setFrame:\0".as_ptr()), bar_rect);
            msg_ptr(bar_layer, sel_registerName(b"setBackgroundColor:\0".as_ptr()), white_color);
            msg_ptr(layer, sel_registerName(b"addSublayer:\0".as_ptr()), bar_layer);
            bar_layers.push(bar_layer);
        }

        let total_dots_width = NOTCH_DOT_COUNT as f64 * NOTCH_DOT_SIZE
            + (NOTCH_DOT_COUNT as f64 - 1.0) * NOTCH_DOT_GAP;
        let dot_start_x = (notch_w - total_dots_width) / 2.0;
        let dot_y = NOTCH_DOT_BOTTOM_INSET;

        let mut dot_layers = Vec::with_capacity(NOTCH_DOT_COUNT);
        for i in 0..NOTCH_DOT_COUNT {
            let dot_layer = objc_msgSend(ca_layer_class, sel_registerName(b"new\0".as_ptr()));
            let dx = dot_start_x + i as f64 * (NOTCH_DOT_SIZE + NOTCH_DOT_GAP);
            let dot_rect = NSRect { x: dx, y: dot_y, w: NOTCH_DOT_SIZE, h: NOTCH_DOT_SIZE };

            type MsgSendSetCGRect = unsafe extern "C" fn(*mut c_void, *mut c_void, NSRect);
            let msg_set_cg_rect: MsgSendSetCGRect = std::mem::transmute(objc_msgSend as *const ());
            msg_set_cg_rect(dot_layer, sel_registerName(b"setFrame:\0".as_ptr()), dot_rect);
            msg_ptr(dot_layer, sel_registerName(b"setBackgroundColor:\0".as_ptr()), white_color);
            msg_f64(dot_layer, sel_registerName(b"setCornerRadius:\0".as_ptr()), NOTCH_DOT_SIZE / 2.0);
            msg_bool(dot_layer, sel_registerName(b"setHidden:\0".as_ptr()), true);
            msg_ptr(layer, sel_registerName(b"addSublayer:\0".as_ptr()), dot_layer);
            dot_layers.push(dot_layer);
        }

        // Show the panel (hidden above screen initially)
        objc_msgSend(panel, sel_registerName(b"orderFrontRegardless\0".as_ptr()));

        // Animate slide down
        let ns_anim = objc_getClass(b"NSAnimationContext\0".as_ptr());
        // beginGrouping
        objc_msgSend(ns_anim, sel_registerName(b"beginGrouping\0".as_ptr()));
        let current_ctx = objc_msgSend(ns_anim, sel_registerName(b"currentContext\0".as_ptr()));
        msg_f64(current_ctx, sel_registerName(b"setDuration:\0".as_ptr()), 0.2);

        // Use a timing function for ease-out
        type MsgSend4F = unsafe extern "C" fn(*mut c_void, *mut c_void, f32, f32, f32, f32) -> *mut c_void;
        let msg_4f: MsgSend4F = std::mem::transmute(objc_msgSend as *const ());
        let ca_timing = objc_getClass(b"CAMediaTimingFunction\0".as_ptr());
        let timing = msg_4f(
            ca_timing,
            sel_registerName(b"functionWithControlPoints::::\0".as_ptr()),
            0.0, 0.0, 0.2, 1.0, // ease-out
        );
        msg_ptr(current_ctx, sel_registerName(b"setTimingFunction:\0".as_ptr()), timing);

        // Animate to final position
        let animator = objc_msgSend(panel, sel_registerName(b"animator\0".as_ptr()));
        let final_rect = NSRect { x, y: y_final, w: notch_w, h: notch_h };
        msg_set_rect(animator, sel_registerName(b"setFrame:display:\0".as_ptr()), final_rect, true);

        objc_msgSend(ns_anim, sel_registerName(b"endGrouping\0".as_ptr()));

        Some(Arc::new(Mutex::new(NotchPanelState {
            panel,
            bar_layers,
            dot_layers,
            content_view,
            width: notch_w,
            height: notch_h,
            eq_bars: vec![0.0; bar_count],
            loading_tick: 0,
        })))
    }
}

/// Update the native notch panel's bar heights.
fn update_notch_bars(state: &mut NotchPanelState, new_bars: &[f32]) {
    let attack = 0.6f32;
    let decay = 0.12f32;
    for (old, &new) in state.eq_bars.iter_mut().zip(new_bars) {
        let factor = if new > *old { attack } else { decay };
        *old += (new - *old) * factor;
    }

    let max_h = NOTCH_BAR_MAX_HEIGHT.min(state.height - NOTCH_BAR_TOP_INSET - 2.0);

    unsafe {
        // Disable implicit animations for smooth manual updates
        let ca_transaction = objc_getClass(b"CATransaction\0".as_ptr());
        objc_msgSend(ca_transaction, sel_registerName(b"begin\0".as_ptr()));
        let msg_bool: MsgSendBool = std::mem::transmute(objc_msgSend as *const ());
        msg_bool(ca_transaction, sel_registerName(b"setDisableActions:\0".as_ptr()), true);
        state.loading_tick = 0;

        let total_bars_width = state.bar_layers.len() as f64 * NOTCH_BAR_WIDTH
            + (state.bar_layers.len() as f64 - 1.0) * NOTCH_BAR_GAP;
        let start_x = (state.width - total_bars_width) / 2.0;

        type MsgSendSetCGRect = unsafe extern "C" fn(*mut c_void, *mut c_void, NSRect);
        let msg_set_cg_rect: MsgSendSetCGRect = std::mem::transmute(objc_msgSend as *const ());

        for &dot_layer in &state.dot_layers {
            msg_bool(dot_layer, sel_registerName(b"setHidden:\0".as_ptr()), true);
        }

        for (i, &magnitude) in state.eq_bars.iter().enumerate() {
            if i >= state.bar_layers.len() { break; }
            let bar_h = (magnitude as f64 * max_h).max(2.0);
            let bx = start_x + i as f64 * (NOTCH_BAR_WIDTH + NOTCH_BAR_GAP);
            let by = state.height - NOTCH_BAR_TOP_INSET - bar_h;
            let rect = NSRect { x: bx, y: by, w: NOTCH_BAR_WIDTH, h: bar_h };
            msg_bool(state.bar_layers[i], sel_registerName(b"setHidden:\0".as_ptr()), false);
            msg_set_cg_rect(state.bar_layers[i], sel_registerName(b"setFrame:\0".as_ptr()), rect);
        }

        objc_msgSend(ca_transaction, sel_registerName(b"commit\0".as_ptr()));
    }
}

fn update_notch_loading(state: &mut NotchPanelState) {
    unsafe {
        let ca_transaction = objc_getClass(b"CATransaction\0".as_ptr());
        objc_msgSend(ca_transaction, sel_registerName(b"begin\0".as_ptr()));
        let msg_bool: MsgSendBool = std::mem::transmute(objc_msgSend as *const ());
        let msg_f32: MsgSendF32 = std::mem::transmute(objc_msgSend as *const ());
        msg_bool(ca_transaction, sel_registerName(b"setDisableActions:\0".as_ptr()), true);

        for &bar_layer in &state.bar_layers {
            msg_bool(bar_layer, sel_registerName(b"setHidden:\0".as_ptr()), true);
        }

        state.loading_tick = state.loading_tick.wrapping_add(1);
        for (i, &dot_layer) in state.dot_layers.iter().enumerate() {
            let phase = ((state.loading_tick as f32 + i as f32 * 10.0) % 30.0) / 30.0;
            let opacity = (phase * std::f32::consts::PI).sin().max(0.12);
            msg_bool(dot_layer, sel_registerName(b"setHidden:\0".as_ptr()), false);
            msg_f32(dot_layer, sel_registerName(b"setOpacity:\0".as_ptr()), opacity);
        }

        objc_msgSend(ca_transaction, sel_registerName(b"commit\0".as_ptr()));
    }
}

/// Close and release the native notch panel.
fn close_notch_panel(state: &NotchPanelState) {
    unsafe {
        objc_msgSend(state.panel, sel_registerName(b"orderOut:\0".as_ptr()));
        // Panel will be released when the Arc<Mutex<NotchPanelState>> is dropped
    }
}

// ---------------------------------------------------------------------------
// OverlayView — GPUI view for floating mode only
// ---------------------------------------------------------------------------

#[derive(PartialEq)]
enum OverlayMode {
    Eq,
    Loading,
}

pub struct OverlayView {
    shared: SharedState,
    mode: OverlayMode,
    eq_bars: Vec<f32>,
    loading_tick: usize,
    overlay_config: OverlayConfig,
}

impl OverlayView {
    fn new(shared: SharedState, overlay_config: OverlayConfig) -> Self {
        let bar_count = match overlay_config.style {
            OverlayStyle::Classic => 16,
            OverlayStyle::Mini => 6,
            OverlayStyle::None => 0,
        };
        Self {
            shared,
            mode: OverlayMode::Eq,
            eq_bars: vec![0.0; bar_count],
            loading_tick: 0,
            overlay_config,
        }
    }

    fn start_animation_timer(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let should_close = this
                    .update(cx, |view, cx| {
                        let phase = view.shared.overlay_phase();
                        match phase {
                            OverlayPhase::Dismissed | OverlayPhase::Hidden => return true,
                            OverlayPhase::Processing => {
                                view.mode = OverlayMode::Loading;
                                view.loading_tick = view.loading_tick.wrapping_add(1);
                            }
                            OverlayPhase::Recording => {
                                view.mode = OverlayMode::Eq;
                                let half = (view.eq_bars.len() + 1) / 2;
                                let new_half = compute_eq_bars(&view.shared, half);
                                let total = view.eq_bars.len();
                                let mut new_bars = Vec::with_capacity(total);
                                for i in (0..total / 2).rev() {
                                    new_bars.push(new_half.get(i).copied().unwrap_or(0.0));
                                }
                                for i in 0..((total + 1) / 2) {
                                    new_bars.push(new_half.get(i).copied().unwrap_or(0.0));
                                }
                                new_bars.truncate(total);
                                let attack = 0.6;
                                let decay = 0.12;
                                for (old, &new) in view.eq_bars.iter_mut().zip(&new_bars) {
                                    let factor = if new > *old { attack } else { decay };
                                    *old += (new - *old) * factor;
                                }
                            }
                        }
                        cx.notify();
                        false
                    })
                    .unwrap_or(true);
                if should_close { break; }
            }
        })
        .detach();
    }
}

impl Render for OverlayView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let w = self.overlay_config.width as f32;
        let h = self.overlay_config.height as f32;
        match self.mode {
            OverlayMode::Eq => render_eq(&self.eq_bars, w, h, self.overlay_config.style),
            OverlayMode::Loading => render_loading(self.loading_tick, w, h),
        }
    }
}

fn render_eq(bars: &[f32], w: f32, h: f32, style: OverlayStyle) -> gpui::Div {
    let (bar_width, gap) = match style {
        OverlayStyle::Classic => (8.0f32, 4.0f32),
        OverlayStyle::Mini => (12.0, 6.0),
        OverlayStyle::None => (0.0, 0.0),
    };
    let max_bar_height = h - 16.0;

    div()
        .w(px(w))
        .h(px(h))
        .flex()
        .items_end()
        .justify_center()
        .gap(px(gap))
        .pb(px(10.0))
        .bg(hsla(0.0, 0.0, 0.06, 0.97))
        .rounded(px(4.0))
        .children(bars.iter().enumerate().map(|(i, &magnitude)| {
            let bar_h = (magnitude * max_bar_height).max(2.0);
            div()
                .id(SharedString::from(format!("bar-{i}")))
                .w(px(bar_width))
                .h(px(bar_h))
                .bg(hsla(0.0, 0.0, 0.78, 0.9))
        }))
}

fn render_loading(tick: usize, w: f32, h: f32) -> gpui::Div {
    div()
        .w(px(w))
        .h(px(h))
        .bg(hsla(0.0, 0.0, 0.06, 0.97))
        .rounded(px(4.0))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .children((0..3).map(move |i| {
            let phase = ((tick as f32 + i as f32 * 10.0) % 30.0) / 30.0;
            let dot_opacity = (phase * std::f32::consts::PI).sin().max(0.12);
            div()
                .id(SharedString::from(format!("dot-{i}")))
                .w(px(6.0))
                .h(px(6.0))
                .rounded(px(3.0))
                .bg(hsla(0.0, 0.0, 0.78, dot_opacity))
        }))
}

// ---------------------------------------------------------------------------
// FFT Computation
// ---------------------------------------------------------------------------

fn compute_eq_bars(shared: &SharedState, bar_count: usize) -> Vec<f32> {
    if bar_count == 0 {
        return vec![];
    }
    let Some(live_ref) = shared.live_audio() else {
        return vec![0.0; bar_count];
    };
    let live = live_ref.lock().unwrap();
    let fft_size: usize = 1024;
    let ring_len = live.ring.len();
    if ring_len == 0 {
        return vec![0.0; bar_count];
    }
    let mut samples = vec![0.0f32; fft_size];
    let effective_pos = live.write_pos % ring_len;
    let start = if effective_pos >= fft_size {
        effective_pos - fft_size
    } else {
        ring_len - (fft_size - effective_pos)
    };
    for i in 0..fft_size {
        samples[i] = live.ring[(start + i) % ring_len];
    }
    let sample_rate = live.sample_rate;
    drop(live);

    use spectrum_analyzer::scaling::divide_by_N_sqrt;
    use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};
    let lo_hz: f32 = 80.0;
    let hi_hz: f32 = 8000.0;
    let spectrum = samples_fft_to_spectrum(
        &samples, sample_rate,
        FrequencyLimit::Range(lo_hz, hi_hz),
        Some(&divide_by_N_sqrt),
    );
    match spectrum {
        Ok(spectrum) => {
            let data = spectrum.data();
            if data.is_empty() { return vec![0.0; bar_count]; }
            let log_lo = lo_hz.ln();
            let log_hi = hi_hz.ln();
            (0..bar_count)
                .map(|i| {
                    let t0 = i as f32 / bar_count as f32;
                    let t1 = (i + 1) as f32 / bar_count as f32;
                    let freq_lo = (log_lo + t0 * (log_hi - log_lo)).exp();
                    let freq_hi = (log_lo + t1 * (log_hi - log_lo)).exp();
                    let max_mag = data.iter()
                        .filter(|(freq, _)| { let f = freq.val(); f >= freq_lo && f < freq_hi })
                        .map(|(_, mag)| mag.val())
                        .fold(0.0f32, f32::max);
                    (max_mag * 20.0).min(1.0)
                })
                .collect()
        }
        Err(_) => vec![0.0; bar_count],
    }
}

// ---------------------------------------------------------------------------
// OverlayController — manages overlay lifecycle
// ---------------------------------------------------------------------------

enum ActiveOverlay {
    Gpui(WindowHandle<OverlayView>),
    Notch(Arc<Mutex<NotchPanelState>>),
}

pub struct OverlayController {
    shared: SharedState,
    active: Option<ActiveOverlay>,
}

impl OverlayController {
    pub fn new(shared: SharedState) -> Self {
        Self { shared, active: None }
    }

    pub fn start_polling(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let should_continue = this
                    .update(cx, |controller, cx| {
                        let phase = controller.shared.overlay_phase();
                        match phase {
                            OverlayPhase::Recording if controller.active.is_none() => {
                                controller.open_overlay(cx);
                            }
                            OverlayPhase::Dismissed if controller.active.is_some() => {
                                controller.close_overlay(cx);
                                controller.shared.set_overlay_phase(OverlayPhase::Hidden);
                            }
                            _ => {}
                        }

                        // Update notch panel bars directly (GPUI view updates itself)
                        if let Some(ActiveOverlay::Notch(ref state)) = controller.active {
                            match phase {
                                OverlayPhase::Recording => {
                                    let half = (NOTCH_BAR_COUNT + 1) / 2;
                                    let new_half = compute_eq_bars(&controller.shared, half);
                                    let mut new_bars = Vec::with_capacity(NOTCH_BAR_COUNT);
                                    for i in (0..NOTCH_BAR_COUNT / 2).rev() {
                                        new_bars.push(new_half.get(i).copied().unwrap_or(0.0));
                                    }
                                    for i in 0..((NOTCH_BAR_COUNT + 1) / 2) {
                                        new_bars.push(new_half.get(i).copied().unwrap_or(0.0));
                                    }
                                    new_bars.truncate(NOTCH_BAR_COUNT);

                                    let mut s = state.lock().unwrap();
                                    update_notch_bars(&mut s, &new_bars);
                                }
                                OverlayPhase::Processing => {
                                    let mut s = state.lock().unwrap();
                                    update_notch_loading(&mut s);
                                }
                                _ => {}
                            }
                        }

                        true
                    })
                    .unwrap_or_else(|_| {
                        eprintln!("[glide] overlay controller: entity released, stopping poll");
                        false
                    });
                if !should_continue { break; }
            }
        })
        .detach();
    }

    fn open_overlay(&mut self, cx: &mut Context<Self>) {
        let config = self.shared.config();
        if config.overlay.style == OverlayStyle::None {
            return;
        }

        match config.overlay.position {
            OverlayPosition::Notch => {
                if let Some(state) = create_notch_panel(&self.shared) {
                    self.active = Some(ActiveOverlay::Notch(state));
                }
            }
            OverlayPosition::Floating => {
                let (screen_w, screen_h) = crate::config::main_display_size();
                let ov = &config.overlay;
                let x = (screen_w as i32 - ov.width as i32) / 2;
                let y = screen_h as i32 - ov.height as i32 - 80;
                let shared = self.shared.clone();
                let overlay_config = ov.clone();

                let handle = cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(x as f32), px(y as f32)),
                            size(px(ov.width as f32), px(ov.height as f32)),
                        ))),
                        kind: WindowKind::PopUp,
                        window_background: WindowBackgroundAppearance::Transparent,
                        titlebar: None,
                        focus: false,
                        is_movable: false,
                        is_resizable: false,
                        ..Default::default()
                    },
                    |_window, cx| {
                        cx.new(|cx| {
                            OverlayView::start_animation_timer(cx);
                            OverlayView::new(shared, overlay_config)
                        })
                    },
                );
                match handle {
                    Ok(h) => self.active = Some(ActiveOverlay::Gpui(h)),
                    Err(e) => eprintln!("[glide] failed to open overlay window: {e}"),
                }
            }
        }
    }

    fn close_overlay(&mut self, cx: &mut Context<Self>) {
        match self.active.take() {
            Some(ActiveOverlay::Gpui(handle)) => {
                let _ = handle.update(cx, |_view, window, _cx| {
                    window.remove_window();
                });
            }
            Some(ActiveOverlay::Notch(state)) => {
                let s = state.lock().unwrap();
                close_notch_panel(&s);
            }
            None => {}
        }
    }
}
