use std::ffi::c_void;
use std::sync::{Arc, Mutex};

pub(super) const NOTCH_WIDTH_FALLBACK: u32 = 180;
const NOTCH_HEIGHT: f64 = 60.0;
const NOTCH_CORNER_RADIUS: f64 = 10.0;
pub(super) const NOTCH_BAR_COUNT: usize = 16;
const NOTCH_BAR_GAP: f64 = 2.0;
const NOTCH_BAR_WIDTH: f64 = 4.0;
const NOTCH_DOT_COUNT: usize = 3;
const NOTCH_DOT_GAP: f64 = 8.0;
const NOTCH_DOT_SIZE: f64 = 6.0;
const NOTCH_BAR_TOP_INSET: f64 = 6.0;
const NOTCH_BAR_MAX_HEIGHT: f64 = 40.0;
const NOTCH_DOT_BOTTOM_INSET: f64 = 12.0;

const GLOW_PADDING: f64 = 28.0;
const GLOW_STROKE_WIDTH: f64 = 5.5;
const GLOW_SHADOW_RADIUS: f64 = 20.0;
const GLOW_CORNER_RADIUS: f64 = 14.0;
const GLOW_ORBIT_DURATION: f64 = 1.4;
const GLOW_COMET_LENGTH: f64 = 120.0;

// ---------------------------------------------------------------------------
// Objective-C / CoreGraphics FFI
// ---------------------------------------------------------------------------

#[link(name = "AppKit", kind = "framework")]
#[link(name = "QuartzCore", kind = "framework")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {}

unsafe extern "C" {
    fn objc_getClass(name: *const u8) -> *mut c_void;
    fn sel_registerName(name: *const u8) -> *mut c_void;
    fn objc_msgSend(receiver: *mut c_void, sel: *mut c_void) -> *mut c_void;
}

unsafe extern "C" {
    fn CGPathCreateMutable() -> *mut c_void;
    fn CGPathMoveToPoint(path: *mut c_void, m: *const c_void, x: f64, y: f64);
    fn CGPathAddLineToPoint(path: *mut c_void, m: *const c_void, x: f64, y: f64);
    fn CGPathAddArcToPoint(
        path: *mut c_void,
        m: *const c_void,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        radius: f64,
    );
    fn CGPathRelease(path: *mut c_void);
}

unsafe fn nsstring_cstr(s: &[u8]) -> *mut c_void {
    unsafe {
        let msg_ptr: MsgSendPtr = std::mem::transmute(objc_msgSend as *const ());
        msg_ptr(
            objc_getClass(b"NSString\0".as_ptr()),
            sel_registerName(b"stringWithUTF8String:\0".as_ptr()),
            s.as_ptr() as *mut c_void,
        )
    }
}

unsafe fn objc_release(obj: *mut c_void) {
    unsafe {
        objc_msgSend(obj, sel_registerName(b"release\0".as_ptr()));
    }
}

type MsgSendF64 = unsafe extern "C" fn(*mut c_void, *mut c_void, f64) -> *mut c_void;
type MsgSendF32 = unsafe extern "C" fn(*mut c_void, *mut c_void, f32) -> *mut c_void;
type MsgSendBool = unsafe extern "C" fn(*mut c_void, *mut c_void, bool) -> *mut c_void;
type MsgSendI64 = unsafe extern "C" fn(*mut c_void, *mut c_void, i64) -> *mut c_void;
type MsgSendU64 = unsafe extern "C" fn(*mut c_void, *mut c_void, u64) -> *mut c_void;
type MsgSendPtr = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
type MsgSendRect = unsafe extern "C" fn(*mut c_void, *mut c_void) -> NSRect;
type MsgSendRectBoolBool =
    unsafe extern "C" fn(*mut c_void, *mut c_void, NSRect, u64, u64, bool) -> *mut c_void;
type MsgSendSetRect = unsafe extern "C" fn(*mut c_void, *mut c_void, NSRect, bool);

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct NSRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

// ---------------------------------------------------------------------------
// Notch panel
// ---------------------------------------------------------------------------

pub(super) struct NotchPanelState {
    panel: *mut c_void,
    bar_layers: Vec<*mut c_void>,
    dot_layers: Vec<*mut c_void>,
    #[allow(dead_code)]
    content_view: *mut c_void,
    width: f64,
    height: f64,
    pub(super) eq_bars: Vec<f32>,
    pub(super) loading_tick: usize,
}

unsafe impl Send for NotchPanelState {}
unsafe impl Sync for NotchPanelState {}

pub(super) fn create_notch_panel(
    bar_rgba: (f64, f64, f64, f64),
) -> Option<Arc<Mutex<NotchPanelState>>> {
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

        let ns_screen = objc_getClass(b"NSScreen\0".as_ptr());
        let main_screen = objc_msgSend(ns_screen, sel_registerName(b"mainScreen\0".as_ptr()));
        if main_screen.is_null() {
            return None;
        }
        let screen_frame = msg_rect(main_screen, sel_registerName(b"frame\0".as_ptr()));

        let x = (screen_frame.w - notch_w) / 2.0;
        let y_final = screen_frame.y + screen_frame.h - notch_h;
        let y_hidden = screen_frame.y + screen_frame.h;

        let ns_panel_class = objc_getClass(b"NSPanel\0".as_ptr());
        let panel = objc_msgSend(ns_panel_class, sel_registerName(b"alloc\0".as_ptr()));
        let content_rect = NSRect {
            x,
            y: y_hidden,
            w: notch_w,
            h: notch_h,
        };
        let panel = msg_init_rect(
            panel,
            sel_registerName(b"initWithContentRect:styleMask:backing:defer:\0".as_ptr()),
            content_rect,
            128,
            2,
            false,
        );
        if panel.is_null() {
            return None;
        }

        let clear_color = objc_msgSend(
            objc_getClass(b"NSColor\0".as_ptr()),
            sel_registerName(b"clearColor\0".as_ptr()),
        );
        msg_ptr(
            panel,
            sel_registerName(b"setBackgroundColor:\0".as_ptr()),
            clear_color,
        );
        msg_bool(panel, sel_registerName(b"setOpaque:\0".as_ptr()), false);
        msg_bool(panel, sel_registerName(b"setHasShadow:\0".as_ptr()), false);
        msg_i64(panel, sel_registerName(b"setLevel:\0".as_ptr()), 1000);
        msg_bool(
            panel,
            sel_registerName(b"setIgnoresMouseEvents:\0".as_ptr()),
            true,
        );
        msg_u64(
            panel,
            sel_registerName(b"setCollectionBehavior:\0".as_ptr()),
            1 << 0,
        );
        msg_bool(
            panel,
            sel_registerName(b"setHidesOnDeactivate:\0".as_ptr()),
            false,
        );

        let ns_view_class = objc_getClass(b"NSView\0".as_ptr());
        let content_view = objc_msgSend(ns_view_class, sel_registerName(b"alloc\0".as_ptr()));
        let content_view = objc_msgSend(content_view, sel_registerName(b"init\0".as_ptr()));
        let view_rect = NSRect {
            x: 0.0,
            y: 0.0,
            w: notch_w,
            h: notch_h,
        };
        msg_set_rect(
            content_view,
            sel_registerName(b"setFrame:\0".as_ptr()),
            view_rect,
            false,
        );
        msg_bool(
            content_view,
            sel_registerName(b"setWantsLayer:\0".as_ptr()),
            true,
        );
        let layer = objc_msgSend(content_view, sel_registerName(b"layer\0".as_ptr()));

        let cg_black = objc_msgSend(
            objc_getClass(b"NSColor\0".as_ptr()),
            sel_registerName(b"blackColor\0".as_ptr()),
        );
        let cg_color = objc_msgSend(cg_black, sel_registerName(b"CGColor\0".as_ptr()));
        msg_ptr(
            layer,
            sel_registerName(b"setBackgroundColor:\0".as_ptr()),
            cg_color,
        );
        msg_f64(
            layer,
            sel_registerName(b"setCornerRadius:\0".as_ptr()),
            NOTCH_CORNER_RADIUS,
        );
        msg_u64(
            layer,
            sel_registerName(b"setMaskedCorners:\0".as_ptr()),
            1 | 2,
        );
        msg_ptr(
            panel,
            sel_registerName(b"setContentView:\0".as_ptr()),
            content_view,
        );
        objc_release(content_view);

        let total_bars_width =
            bar_count as f64 * NOTCH_BAR_WIDTH + (bar_count as f64 - 1.0) * NOTCH_BAR_GAP;
        let start_x = (notch_w - total_bars_width) / 2.0;

        let ca_layer_class = objc_getClass(b"CALayer\0".as_ptr());
        let bar_cg_color = {
            type MsgSendRGBA =
                unsafe extern "C" fn(*mut c_void, *mut c_void, f64, f64, f64, f64) -> *mut c_void;
            let msg_rgba: MsgSendRGBA = std::mem::transmute(objc_msgSend as *const ());
            let (br, bg, bb, ba) = bar_rgba;
            let ns_color = msg_rgba(
                objc_getClass(b"NSColor\0".as_ptr()),
                sel_registerName(b"colorWithRed:green:blue:alpha:\0".as_ptr()),
                br,
                bg,
                bb,
                ba,
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
            msg_set_cg_rect(
                bar_layer,
                sel_registerName(b"setFrame:\0".as_ptr()),
                bar_rect,
            );
            msg_ptr(
                bar_layer,
                sel_registerName(b"setBackgroundColor:\0".as_ptr()),
                bar_cg_color,
            );
            msg_ptr(
                layer,
                sel_registerName(b"addSublayer:\0".as_ptr()),
                bar_layer,
            );
            objc_release(bar_layer);
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
            let dot_rect = NSRect {
                x: dx,
                y: dot_y,
                w: NOTCH_DOT_SIZE,
                h: NOTCH_DOT_SIZE,
            };
            type MsgSendSetCGRect = unsafe extern "C" fn(*mut c_void, *mut c_void, NSRect);
            let msg_set_cg_rect: MsgSendSetCGRect = std::mem::transmute(objc_msgSend as *const ());
            msg_set_cg_rect(
                dot_layer,
                sel_registerName(b"setFrame:\0".as_ptr()),
                dot_rect,
            );
            msg_ptr(
                dot_layer,
                sel_registerName(b"setBackgroundColor:\0".as_ptr()),
                bar_cg_color,
            );
            msg_f64(
                dot_layer,
                sel_registerName(b"setCornerRadius:\0".as_ptr()),
                NOTCH_DOT_SIZE / 2.0,
            );
            msg_bool(dot_layer, sel_registerName(b"setHidden:\0".as_ptr()), true);
            msg_ptr(
                layer,
                sel_registerName(b"addSublayer:\0".as_ptr()),
                dot_layer,
            );
            objc_release(dot_layer);
            dot_layers.push(dot_layer);
        }

        objc_msgSend(panel, sel_registerName(b"orderFrontRegardless\0".as_ptr()));

        let ns_anim = objc_getClass(b"NSAnimationContext\0".as_ptr());
        objc_msgSend(ns_anim, sel_registerName(b"beginGrouping\0".as_ptr()));
        let current_ctx = objc_msgSend(ns_anim, sel_registerName(b"currentContext\0".as_ptr()));
        msg_f64(
            current_ctx,
            sel_registerName(b"setDuration:\0".as_ptr()),
            0.2,
        );
        type MsgSend4F =
            unsafe extern "C" fn(*mut c_void, *mut c_void, f32, f32, f32, f32) -> *mut c_void;
        let msg_4f: MsgSend4F = std::mem::transmute(objc_msgSend as *const ());
        let timing = msg_4f(
            objc_getClass(b"CAMediaTimingFunction\0".as_ptr()),
            sel_registerName(b"functionWithControlPoints::::\0".as_ptr()),
            0.0,
            0.0,
            0.2,
            1.0,
        );
        msg_ptr(
            current_ctx,
            sel_registerName(b"setTimingFunction:\0".as_ptr()),
            timing,
        );
        let animator = objc_msgSend(panel, sel_registerName(b"animator\0".as_ptr()));
        let final_rect = NSRect {
            x,
            y: y_final,
            w: notch_w,
            h: notch_h,
        };
        msg_set_rect(
            animator,
            sel_registerName(b"setFrame:display:\0".as_ptr()),
            final_rect,
            true,
        );
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

pub(super) fn update_notch_bars(state: &mut NotchPanelState, new_bars: &[f32]) {
    let attack = 0.6f32;
    let decay = 0.12f32;
    for (old, &new) in state.eq_bars.iter_mut().zip(new_bars) {
        let factor = if new > *old { attack } else { decay };
        *old += (new - *old) * factor;
    }
    let max_h = NOTCH_BAR_MAX_HEIGHT.min(state.height - NOTCH_BAR_TOP_INSET - 2.0);

    unsafe {
        let ca_transaction = objc_getClass(b"CATransaction\0".as_ptr());
        objc_msgSend(ca_transaction, sel_registerName(b"begin\0".as_ptr()));
        let msg_bool: MsgSendBool = std::mem::transmute(objc_msgSend as *const ());
        msg_bool(
            ca_transaction,
            sel_registerName(b"setDisableActions:\0".as_ptr()),
            true,
        );
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
            if i >= state.bar_layers.len() {
                break;
            }
            let bar_h = (magnitude as f64 * max_h).max(2.0);
            let bx = start_x + i as f64 * (NOTCH_BAR_WIDTH + NOTCH_BAR_GAP);
            let by = state.height - NOTCH_BAR_TOP_INSET - bar_h;
            let rect = NSRect {
                x: bx,
                y: by,
                w: NOTCH_BAR_WIDTH,
                h: bar_h,
            };
            msg_bool(
                state.bar_layers[i],
                sel_registerName(b"setHidden:\0".as_ptr()),
                false,
            );
            msg_set_cg_rect(
                state.bar_layers[i],
                sel_registerName(b"setFrame:\0".as_ptr()),
                rect,
            );
        }
        objc_msgSend(ca_transaction, sel_registerName(b"commit\0".as_ptr()));
    }
}

pub(super) fn update_notch_loading(state: &mut NotchPanelState) {
    unsafe {
        let ca_transaction = objc_getClass(b"CATransaction\0".as_ptr());
        objc_msgSend(ca_transaction, sel_registerName(b"begin\0".as_ptr()));
        let msg_bool: MsgSendBool = std::mem::transmute(objc_msgSend as *const ());
        let msg_f32: MsgSendF32 = std::mem::transmute(objc_msgSend as *const ());
        msg_bool(
            ca_transaction,
            sel_registerName(b"setDisableActions:\0".as_ptr()),
            true,
        );

        for &bar_layer in &state.bar_layers {
            msg_bool(bar_layer, sel_registerName(b"setHidden:\0".as_ptr()), true);
        }
        state.loading_tick = state.loading_tick.wrapping_add(1);
        for (i, &dot_layer) in state.dot_layers.iter().enumerate() {
            let phase = ((state.loading_tick as f32 + i as f32 * 10.0) % 30.0) / 30.0;
            let opacity = (phase * std::f32::consts::PI).sin().max(0.12);
            msg_bool(dot_layer, sel_registerName(b"setHidden:\0".as_ptr()), false);
            msg_f32(
                dot_layer,
                sel_registerName(b"setOpacity:\0".as_ptr()),
                opacity,
            );
        }
        objc_msgSend(ca_transaction, sel_registerName(b"commit\0".as_ptr()));
    }
}

pub(super) fn close_notch_panel(state: &NotchPanelState) {
    unsafe {
        objc_msgSend(state.panel, sel_registerName(b"orderOut:\0".as_ptr()));
        objc_release(state.panel);
    }
}

// ---------------------------------------------------------------------------
// Glow panel
// ---------------------------------------------------------------------------

pub(super) struct NotchGlowState {
    panel: *mut c_void,
}

unsafe impl Send for NotchGlowState {}
unsafe impl Sync for NotchGlowState {}

pub(super) fn create_notch_glow_panel(
    glow_rgb: Option<(f64, f64, f64)>,
) -> Option<Arc<Mutex<NotchGlowState>>> {
    let (notch_w, notch_h) = crate::config::notch_dimensions()?;
    let panel_w = notch_w + 2.0 * GLOW_PADDING;
    let panel_h = notch_h + GLOW_PADDING;
    let r = GLOW_CORNER_RADIUS;

    unsafe {
        let msg_ptr: MsgSendPtr = std::mem::transmute(objc_msgSend as *const ());
        let msg_bool: MsgSendBool = std::mem::transmute(objc_msgSend as *const ());
        let msg_i64: MsgSendI64 = std::mem::transmute(objc_msgSend as *const ());
        let msg_u64: MsgSendU64 = std::mem::transmute(objc_msgSend as *const ());
        let msg_f64: MsgSendF64 = std::mem::transmute(objc_msgSend as *const ());
        let msg_f32: MsgSendF32 = std::mem::transmute(objc_msgSend as *const ());
        let msg_rect: MsgSendRect = std::mem::transmute(objc_msgSend as *const ());
        let msg_init_rect: MsgSendRectBoolBool = std::mem::transmute(objc_msgSend as *const ());
        let msg_set_rect: MsgSendSetRect = std::mem::transmute(objc_msgSend as *const ());

        let ns_screen = objc_getClass(b"NSScreen\0".as_ptr());
        let main_screen = objc_msgSend(ns_screen, sel_registerName(b"mainScreen\0".as_ptr()));
        if main_screen.is_null() {
            return None;
        }
        let screen_frame = msg_rect(main_screen, sel_registerName(b"frame\0".as_ptr()));

        let x = (screen_frame.w - panel_w) / 2.0;
        let y_final = screen_frame.y + screen_frame.h - panel_h;
        let y_hidden = screen_frame.y + screen_frame.h;
        let ns_panel_class = objc_getClass(b"NSPanel\0".as_ptr());
        let panel = objc_msgSend(ns_panel_class, sel_registerName(b"alloc\0".as_ptr()));
        let content_rect = NSRect {
            x,
            y: y_hidden,
            w: panel_w,
            h: panel_h,
        };
        let panel = msg_init_rect(
            panel,
            sel_registerName(b"initWithContentRect:styleMask:backing:defer:\0".as_ptr()),
            content_rect,
            128,
            2,
            false,
        );
        if panel.is_null() {
            return None;
        }

        let clear_color = objc_msgSend(
            objc_getClass(b"NSColor\0".as_ptr()),
            sel_registerName(b"clearColor\0".as_ptr()),
        );
        msg_ptr(
            panel,
            sel_registerName(b"setBackgroundColor:\0".as_ptr()),
            clear_color,
        );
        msg_bool(panel, sel_registerName(b"setOpaque:\0".as_ptr()), false);
        msg_bool(panel, sel_registerName(b"setHasShadow:\0".as_ptr()), false);
        msg_i64(panel, sel_registerName(b"setLevel:\0".as_ptr()), 1000);
        msg_bool(
            panel,
            sel_registerName(b"setIgnoresMouseEvents:\0".as_ptr()),
            true,
        );
        msg_u64(
            panel,
            sel_registerName(b"setCollectionBehavior:\0".as_ptr()),
            1 << 0,
        );
        msg_bool(
            panel,
            sel_registerName(b"setHidesOnDeactivate:\0".as_ptr()),
            false,
        );

        let ns_view_class = objc_getClass(b"NSView\0".as_ptr());
        let content_view = objc_msgSend(ns_view_class, sel_registerName(b"alloc\0".as_ptr()));
        let content_view = objc_msgSend(content_view, sel_registerName(b"init\0".as_ptr()));
        let view_rect = NSRect {
            x: 0.0,
            y: 0.0,
            w: panel_w,
            h: panel_h,
        };
        msg_set_rect(
            content_view,
            sel_registerName(b"setFrame:\0".as_ptr()),
            view_rect,
            false,
        );
        msg_bool(
            content_view,
            sel_registerName(b"setWantsLayer:\0".as_ptr()),
            true,
        );
        let root_layer = objc_msgSend(content_view, sel_registerName(b"layer\0".as_ptr()));
        let clear_cg = objc_msgSend(clear_color, sel_registerName(b"CGColor\0".as_ptr()));
        msg_ptr(
            root_layer,
            sel_registerName(b"setBackgroundColor:\0".as_ptr()),
            clear_cg,
        );
        msg_ptr(
            panel,
            sel_registerName(b"setContentView:\0".as_ptr()),
            content_view,
        );
        objc_release(content_view);

        let left = GLOW_PADDING;
        let right = panel_w - GLOW_PADDING;
        let top = panel_h;
        let bottom = GLOW_PADDING;

        let cg_path = CGPathCreateMutable();
        let null_ptr = std::ptr::null::<c_void>();
        CGPathMoveToPoint(cg_path, null_ptr, left, top);
        CGPathAddLineToPoint(cg_path, null_ptr, left, bottom + r);
        CGPathAddArcToPoint(cg_path, null_ptr, left, bottom, left + r, bottom, r);
        CGPathAddLineToPoint(cg_path, null_ptr, right - r, bottom);
        CGPathAddArcToPoint(cg_path, null_ptr, right, bottom, right, bottom + r, r);
        CGPathAddLineToPoint(cg_path, null_ptr, right, top);

        type MsgSendCGSize = unsafe extern "C" fn(*mut c_void, *mut c_void, f64, f64);
        let msg_cgsize: MsgSendCGSize = std::mem::transmute(objc_msgSend as *const ());
        type MsgSendRGBA =
            unsafe extern "C" fn(*mut c_void, *mut c_void, f64, f64, f64, f64) -> *mut c_void;
        let msg_rgba: MsgSendRGBA = std::mem::transmute(objc_msgSend as *const ());

        let ns_color_class = objc_getClass(b"NSColor\0".as_ptr());
        let rgba_sel = sel_registerName(b"colorWithRed:green:blue:alpha:\0".as_ptr());

        let rainbow = glow_rgb.is_none();
        let (gr, gg, gb) = glow_rgb.unwrap_or((0.4, 0.7, 1.0));
        let dim_color = msg_rgba(ns_color_class, rgba_sel, gr, gg, gb, 0.20);
        let dim_cg = objc_msgSend(dim_color, sel_registerName(b"CGColor\0".as_ptr()));
        let bright_color = msg_rgba(ns_color_class, rgba_sel, gr, gg, gb, 1.0);
        let bright_cg = objc_msgSend(bright_color, sel_registerName(b"CGColor\0".as_ptr()));

        let shape_class = objc_getClass(b"CAShapeLayer\0".as_ptr());

        let glow_layer = objc_msgSend(shape_class, sel_registerName(b"new\0".as_ptr()));
        msg_ptr(
            glow_layer,
            sel_registerName(b"setPath:\0".as_ptr()),
            cg_path,
        );
        msg_ptr(
            glow_layer,
            sel_registerName(b"setStrokeColor:\0".as_ptr()),
            dim_cg,
        );
        msg_ptr(
            glow_layer,
            sel_registerName(b"setFillColor:\0".as_ptr()),
            std::ptr::null_mut(),
        );
        msg_f64(
            glow_layer,
            sel_registerName(b"setLineWidth:\0".as_ptr()),
            GLOW_STROKE_WIDTH + 0.5,
        );
        msg_ptr(
            glow_layer,
            sel_registerName(b"setShadowColor:\0".as_ptr()),
            dim_cg,
        );
        msg_f64(
            glow_layer,
            sel_registerName(b"setShadowRadius:\0".as_ptr()),
            GLOW_SHADOW_RADIUS,
        );
        msg_f32(
            glow_layer,
            sel_registerName(b"setShadowOpacity:\0".as_ptr()),
            0.6,
        );
        msg_cgsize(
            glow_layer,
            sel_registerName(b"setShadowOffset:\0".as_ptr()),
            0.0,
            0.0,
        );
        msg_ptr(
            root_layer,
            sel_registerName(b"addSublayer:\0".as_ptr()),
            glow_layer,
        );

        // Rainbow multi-color gradient for Slate accent
        // Shows ALL rainbow colors simultaneously via a scrolling gradient
        // masked to the notch stroke path.
        if rainbow {
            type MsgSendArrayObjs = unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                *const *mut c_void,
                u64,
            ) -> *mut c_void;
            let msg_array: MsgSendArrayObjs = std::mem::transmute(objc_msgSend as *const ());
            let ns_array_class = objc_getClass(b"NSArray\0".as_ptr());
            let arr_sel = sel_registerName(b"arrayWithObjects:count:\0".as_ptr());
            let cg_color_sel = sel_registerName(b"CGColor\0".as_ptr());
            type MsgSendSetCGRect2 = unsafe extern "C" fn(*mut c_void, *mut c_void, NSRect);
            let msg_set_cg_rect2: MsgSendSetCGRect2 =
                std::mem::transmute(objc_msgSend as *const ());
            type MsgSendSetCGPoint2 = unsafe extern "C" fn(*mut c_void, *mut c_void, f64, f64);
            let msg_set_point2: MsgSendSetCGPoint2 =
                std::mem::transmute(objc_msgSend as *const ());
            type MsgSendPtrPtr2 = unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                *mut c_void,
                *mut c_void,
            ) -> *mut c_void;
            let msg_ptr_ptr2: MsgSendPtrPtr2 = std::mem::transmute(objc_msgSend as *const ());

            let rainbow_specs: [(f64, f64, f64); 7] = [
                (1.0, 0.3, 0.3), // red
                (1.0, 0.6, 0.2), // orange
                (0.9, 1.0, 0.3), // yellow
                (0.3, 1.0, 0.5), // green
                (0.3, 0.8, 1.0), // cyan
                (0.4, 0.4, 1.0), // blue
                (0.8, 0.3, 1.0), // purple
            ];

            // --- Container layer for gradient + path mask ---
            let ca_layer_class = objc_getClass(b"CALayer\0".as_ptr());
            let container = objc_msgSend(ca_layer_class, sel_registerName(b"new\0".as_ptr()));
            let container_rect = NSRect {
                x: 0.0,
                y: 0.0,
                w: panel_w,
                h: panel_h,
            };
            msg_set_cg_rect2(
                container,
                sel_registerName(b"setFrame:\0".as_ptr()),
                container_rect,
            );

            // --- Rainbow gradient (3x width for seamless scroll loop) ---
            let rainbow_grad_class = objc_getClass(b"CAGradientLayer\0".as_ptr());
            let rainbow_grad =
                objc_msgSend(rainbow_grad_class, sel_registerName(b"new\0".as_ptr()));
            let rainbow_grad_w = panel_w * 3.0;
            let rainbow_grad_rect = NSRect {
                x: 0.0,
                y: 0.0,
                w: rainbow_grad_w,
                h: panel_h,
            };
            msg_set_cg_rect2(
                rainbow_grad,
                sel_registerName(b"setFrame:\0".as_ptr()),
                rainbow_grad_rect,
            );
            msg_set_point2(
                rainbow_grad,
                sel_registerName(b"setStartPoint:\0".as_ptr()),
                0.0,
                0.5,
            );
            msg_set_point2(
                rainbow_grad,
                sel_registerName(b"setEndPoint:\0".as_ptr()),
                1.0,
                0.5,
            );

            // Build gradient colors: full rainbow repeated 3x
            let mut grad_colors: Vec<*mut c_void> = Vec::new();
            for _ in 0..3 {
                for &(cr, cg_c, cb) in &rainbow_specs {
                    let c = msg_rgba(ns_color_class, rgba_sel, cr, cg_c, cb, 0.5);
                    grad_colors.push(objc_msgSend(c, cg_color_sel));
                }
            }
            let close_c = msg_rgba(
                ns_color_class,
                rgba_sel,
                rainbow_specs[0].0,
                rainbow_specs[0].1,
                rainbow_specs[0].2,
                0.5,
            );
            grad_colors.push(objc_msgSend(close_c, cg_color_sel));

            let grad_colors_arr = msg_array(
                ns_array_class,
                arr_sel,
                grad_colors.as_ptr(),
                grad_colors.len() as u64,
            );
            msg_ptr(
                rainbow_grad,
                sel_registerName(b"setColors:\0".as_ptr()),
                grad_colors_arr,
            );

            msg_ptr(
                container,
                sel_registerName(b"addSublayer:\0".as_ptr()),
                rainbow_grad,
            );
            objc_release(rainbow_grad);

            // --- Mask: CAShapeLayer with the notch path (stroke only) ---
            let mask_shape = objc_msgSend(shape_class, sel_registerName(b"new\0".as_ptr()));
            msg_ptr(
                mask_shape,
                sel_registerName(b"setPath:\0".as_ptr()),
                cg_path,
            );
            let white_mask = msg_rgba(ns_color_class, rgba_sel, 1.0, 1.0, 1.0, 1.0);
            let white_mask_cg = objc_msgSend(white_mask, sel_registerName(b"CGColor\0".as_ptr()));
            msg_ptr(
                mask_shape,
                sel_registerName(b"setStrokeColor:\0".as_ptr()),
                white_mask_cg,
            );
            msg_ptr(
                mask_shape,
                sel_registerName(b"setFillColor:\0".as_ptr()),
                std::ptr::null_mut(),
            );
            msg_f64(
                mask_shape,
                sel_registerName(b"setLineWidth:\0".as_ptr()),
                GLOW_STROKE_WIDTH + 6.0,
            );
            msg_ptr(
                container,
                sel_registerName(b"setMask:\0".as_ptr()),
                mask_shape,
            );
            objc_release(mask_shape);

            msg_ptr(
                root_layer,
                sel_registerName(b"addSublayer:\0".as_ptr()),
                container,
            );
            objc_release(container);

            // --- Animate gradient scroll so rainbow flows along the path ---
            let rb_anim_class = objc_getClass(b"CABasicAnimation\0".as_ptr());
            let rb_ns_number = objc_getClass(b"NSNumber\0".as_ptr());
            let scroll_anim = msg_ptr(
                rb_anim_class,
                sel_registerName(b"animationWithKeyPath:\0".as_ptr()),
                nsstring_cstr(b"position.x\0"),
            );
            let grad_center_x = rainbow_grad_w / 2.0;
            msg_ptr(
                scroll_anim,
                sel_registerName(b"setFromValue:\0".as_ptr()),
                msg_f64(
                    rb_ns_number,
                    sel_registerName(b"numberWithDouble:\0".as_ptr()),
                    grad_center_x,
                ),
            );
            msg_ptr(
                scroll_anim,
                sel_registerName(b"setToValue:\0".as_ptr()),
                msg_f64(
                    rb_ns_number,
                    sel_registerName(b"numberWithDouble:\0".as_ptr()),
                    grad_center_x - panel_w,
                ),
            );
            msg_f64(
                scroll_anim,
                sel_registerName(b"setDuration:\0".as_ptr()),
                3.0,
            );
            msg_f32(
                scroll_anim,
                sel_registerName(b"setRepeatCount:\0".as_ptr()),
                f32::MAX,
            );
            msg_ptr_ptr2(
                rainbow_grad,
                sel_registerName(b"addAnimation:forKey:\0".as_ptr()),
                scroll_anim,
                nsstring_cstr(b"rainbowScroll\0"),
            );

            // --- Shadow still cycles on glow_layer for ambient glow ---
            let ca_kf_class = objc_getClass(b"CAKeyframeAnimation\0".as_ptr());
            let anim_kp_sel = sel_registerName(b"animationWithKeyPath:\0".as_ptr());
            let bright_rainbow: Vec<*mut c_void> = rainbow_specs
                .iter()
                .chain(std::iter::once(&rainbow_specs[0]))
                .map(|&(r, g, b)| {
                    let c = msg_rgba(ns_color_class, rgba_sel, r, g, b, 0.7);
                    objc_msgSend(c, cg_color_sel)
                })
                .collect();
            let shadow_anim =
                msg_ptr(ca_kf_class, anim_kp_sel, nsstring_cstr(b"shadowColor\0"));
            let bright_arr = msg_array(
                ns_array_class,
                arr_sel,
                bright_rainbow.as_ptr(),
                bright_rainbow.len() as u64,
            );
            msg_ptr(
                shadow_anim,
                sel_registerName(b"setValues:\0".as_ptr()),
                bright_arr,
            );
            msg_f64(
                shadow_anim,
                sel_registerName(b"setDuration:\0".as_ptr()),
                4.0,
            );
            msg_f32(
                shadow_anim,
                sel_registerName(b"setRepeatCount:\0".as_ptr()),
                f32::MAX,
            );
            msg_ptr_ptr2(
                glow_layer,
                sel_registerName(b"addAnimation:forKey:\0".as_ptr()),
                shadow_anim,
                nsstring_cstr(b"rainbowShadow\0"),
            );

            // Dim the base glow_layer stroke since the gradient provides the color
            let subdued = msg_rgba(ns_color_class, rgba_sel, 0.5, 0.5, 0.5, 0.08);
            let subdued_cg = objc_msgSend(subdued, sel_registerName(b"CGColor\0".as_ptr()));
            msg_ptr(
                glow_layer,
                sel_registerName(b"setStrokeColor:\0".as_ptr()),
                subdued_cg,
            );
        }
        objc_release(glow_layer);

        let ns_number = objc_getClass(b"NSNumber\0".as_ptr());
        let ca_anim_class = objc_getClass(b"CABasicAnimation\0".as_ptr());
        type MsgSendPtrPtr =
            unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
        let msg_ptr_ptr: MsgSendPtrPtr = std::mem::transmute(objc_msgSend as *const ());
        type MsgSendSetCGRect = unsafe extern "C" fn(*mut c_void, *mut c_void, NSRect);
        let msg_set_cg_rect: MsgSendSetCGRect = std::mem::transmute(objc_msgSend as *const ());
        type MsgSendSetCGPoint = unsafe extern "C" fn(*mut c_void, *mut c_void, f64, f64);
        let msg_set_point: MsgSendSetCGPoint = std::mem::transmute(objc_msgSend as *const ());
        type MsgSendArrayObjs =
            unsafe extern "C" fn(*mut c_void, *mut c_void, *const *mut c_void, u64) -> *mut c_void;
        let msg_array: MsgSendArrayObjs = std::mem::transmute(objc_msgSend as *const ());

        // Comet uses white in rainbow mode, accent color otherwise
        let comet_cg = if rainbow {
            let white_color = msg_rgba(ns_color_class, rgba_sel, 1.0, 1.0, 1.0, 1.0);
            objc_msgSend(white_color, sel_registerName(b"CGColor\0".as_ptr()))
        } else {
            bright_cg
        };

        let comet = objc_msgSend(shape_class, sel_registerName(b"new\0".as_ptr()));
        msg_ptr(comet, sel_registerName(b"setPath:\0".as_ptr()), cg_path);
        msg_ptr(
            comet,
            sel_registerName(b"setStrokeColor:\0".as_ptr()),
            comet_cg,
        );
        msg_ptr(
            comet,
            sel_registerName(b"setFillColor:\0".as_ptr()),
            std::ptr::null_mut(),
        );
        msg_f64(
            comet,
            sel_registerName(b"setLineWidth:\0".as_ptr()),
            GLOW_STROKE_WIDTH + 2.0,
        );
        msg_ptr(
            comet,
            sel_registerName(b"setLineCap:\0".as_ptr()),
            nsstring_cstr(b"round\0"),
        );
        msg_ptr(
            comet,
            sel_registerName(b"setShadowColor:\0".as_ptr()),
            comet_cg,
        );
        msg_f64(
            comet,
            sel_registerName(b"setShadowRadius:\0".as_ptr()),
            GLOW_SHADOW_RADIUS + 6.0,
        );
        msg_f32(
            comet,
            sel_registerName(b"setShadowOpacity:\0".as_ptr()),
            1.0,
        );
        msg_cgsize(
            comet,
            sel_registerName(b"setShadowOffset:\0".as_ptr()),
            0.0,
            0.0,
        );

        let grad_class = objc_getClass(b"CAGradientLayer\0".as_ptr());
        let grad = objc_msgSend(grad_class, sel_registerName(b"new\0".as_ptr()));
        let grad_w = panel_w * 3.0;
        let grad_rect = NSRect {
            x: -panel_w,
            y: 0.0,
            w: grad_w,
            h: panel_h,
        };
        msg_set_cg_rect(grad, sel_registerName(b"setFrame:\0".as_ptr()), grad_rect);
        msg_set_point(
            grad,
            sel_registerName(b"setStartPoint:\0".as_ptr()),
            0.0,
            0.5,
        );
        msg_set_point(grad, sel_registerName(b"setEndPoint:\0".as_ptr()), 1.0, 0.5);

        let clear_cg = objc_msgSend(
            objc_msgSend(ns_color_class, sel_registerName(b"clearColor\0".as_ptr())),
            sel_registerName(b"CGColor\0".as_ptr()),
        );
        let white_cg = objc_msgSend(
            objc_msgSend(ns_color_class, sel_registerName(b"whiteColor\0".as_ptr())),
            sel_registerName(b"CGColor\0".as_ptr()),
        );
        let colors: [*mut c_void; 5] = [clear_cg, clear_cg, white_cg, clear_cg, clear_cg];
        let colors_arr = msg_array(
            objc_getClass(b"NSArray\0".as_ptr()),
            sel_registerName(b"arrayWithObjects:count:\0".as_ptr()),
            colors.as_ptr(),
            5,
        );
        msg_ptr(grad, sel_registerName(b"setColors:\0".as_ptr()), colors_arr);

        let spot_half = (GLOW_COMET_LENGTH / grad_w) / 2.0;
        let center = 0.5;
        let locs: [*mut c_void; 5] = [
            msg_f64(
                ns_number,
                sel_registerName(b"numberWithDouble:\0".as_ptr()),
                0.0,
            ),
            msg_f64(
                ns_number,
                sel_registerName(b"numberWithDouble:\0".as_ptr()),
                center - spot_half,
            ),
            msg_f64(
                ns_number,
                sel_registerName(b"numberWithDouble:\0".as_ptr()),
                center,
            ),
            msg_f64(
                ns_number,
                sel_registerName(b"numberWithDouble:\0".as_ptr()),
                center + spot_half,
            ),
            msg_f64(
                ns_number,
                sel_registerName(b"numberWithDouble:\0".as_ptr()),
                1.0,
            ),
        ];
        let locs_arr = msg_array(
            objc_getClass(b"NSArray\0".as_ptr()),
            sel_registerName(b"arrayWithObjects:count:\0".as_ptr()),
            locs.as_ptr(),
            5,
        );
        msg_ptr(
            grad,
            sel_registerName(b"setLocations:\0".as_ptr()),
            locs_arr,
        );
        msg_ptr(comet, sel_registerName(b"setMask:\0".as_ptr()), grad);
        msg_ptr(
            root_layer,
            sel_registerName(b"addSublayer:\0".as_ptr()),
            comet,
        );

        let timing = msg_ptr(
            objc_getClass(b"CAMediaTimingFunction\0".as_ptr()),
            sel_registerName(b"functionWithName:\0".as_ptr()),
            nsstring_cstr(b"easeInEaseOut\0"),
        );
        let anim = msg_ptr(
            ca_anim_class,
            sel_registerName(b"animationWithKeyPath:\0".as_ptr()),
            nsstring_cstr(b"position.x\0"),
        );
        msg_ptr(
            anim,
            sel_registerName(b"setFromValue:\0".as_ptr()),
            msg_f64(
                ns_number,
                sel_registerName(b"numberWithDouble:\0".as_ptr()),
                0.0,
            ),
        );
        msg_ptr(
            anim,
            sel_registerName(b"setToValue:\0".as_ptr()),
            msg_f64(
                ns_number,
                sel_registerName(b"numberWithDouble:\0".as_ptr()),
                panel_w,
            ),
        );
        msg_f64(
            anim,
            sel_registerName(b"setDuration:\0".as_ptr()),
            GLOW_ORBIT_DURATION,
        );
        msg_f32(
            anim,
            sel_registerName(b"setRepeatCount:\0".as_ptr()),
            f32::MAX,
        );
        msg_bool(anim, sel_registerName(b"setAutoreverses:\0".as_ptr()), true);
        msg_ptr(
            anim,
            sel_registerName(b"setTimingFunction:\0".as_ptr()),
            timing,
        );
        msg_ptr_ptr(
            grad,
            sel_registerName(b"addAnimation:forKey:\0".as_ptr()),
            anim,
            nsstring_cstr(b"slide\0"),
        );
        objc_release(grad);
        objc_release(comet);

        CGPathRelease(cg_path);

        objc_msgSend(panel, sel_registerName(b"orderFrontRegardless\0".as_ptr()));
        let ns_anim = objc_getClass(b"NSAnimationContext\0".as_ptr());
        objc_msgSend(ns_anim, sel_registerName(b"beginGrouping\0".as_ptr()));
        let current_ctx = objc_msgSend(ns_anim, sel_registerName(b"currentContext\0".as_ptr()));
        msg_f64(
            current_ctx,
            sel_registerName(b"setDuration:\0".as_ptr()),
            0.2,
        );
        type MsgSend4F =
            unsafe extern "C" fn(*mut c_void, *mut c_void, f32, f32, f32, f32) -> *mut c_void;
        let msg_4f: MsgSend4F = std::mem::transmute(objc_msgSend as *const ());
        let slide_timing = msg_4f(
            objc_getClass(b"CAMediaTimingFunction\0".as_ptr()),
            sel_registerName(b"functionWithControlPoints::::\0".as_ptr()),
            0.0,
            0.0,
            0.2,
            1.0,
        );
        msg_ptr(
            current_ctx,
            sel_registerName(b"setTimingFunction:\0".as_ptr()),
            slide_timing,
        );
        let animator = objc_msgSend(panel, sel_registerName(b"animator\0".as_ptr()));
        let final_rect = NSRect {
            x,
            y: y_final,
            w: panel_w,
            h: panel_h,
        };
        msg_set_rect(
            animator,
            sel_registerName(b"setFrame:display:\0".as_ptr()),
            final_rect,
            true,
        );
        objc_msgSend(ns_anim, sel_registerName(b"endGrouping\0".as_ptr()));

        Some(Arc::new(Mutex::new(NotchGlowState { panel })))
    }
}

pub(super) fn close_notch_glow_panel(state: &NotchGlowState) {
    unsafe {
        objc_msgSend(state.panel, sel_registerName(b"orderOut:\0".as_ptr()));
        objc_release(state.panel);
    }
}
