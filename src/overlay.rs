use std::time::Duration;

use gpui::{
    div, hsla, point, px, size, AppContext, Bounds, Context, Entity, Global,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions,
};

/// App-lifetime handle that keeps the OverlayController entity alive via GPUI's global storage.
#[allow(dead_code)] // The Entity is kept alive by its ref count, not by field access
pub struct OverlayHandle(pub Entity<OverlayController>);
impl Global for OverlayHandle {}

use crate::app::{OverlayPhase, SharedState};
use crate::config::{OverlayConfig, OverlayStyle};

// ---------------------------------------------------------------------------
// OverlayView — the root view rendered inside the overlay window
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
                    .timer(Duration::from_millis(33)) // ~30fps
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
                                // Compute half the bars from FFT, then mirror
                                let half = (view.eq_bars.len() + 1) / 2;
                                let new_half = compute_eq_bars(
                                    &view.shared,
                                    half,
                                );
                                // Mirror: [high_freq..low_freq, low_freq..high_freq]
                                let total = view.eq_bars.len();
                                let mut new_bars = Vec::with_capacity(total);
                                // Left half: reversed (high freq on outside, low freq center)
                                for i in (0..total / 2).rev() {
                                    new_bars.push(new_half.get(i).copied().unwrap_or(0.0));
                                }
                                // Right half: normal order (low freq center, high freq outside)
                                for i in 0..((total + 1) / 2) {
                                    new_bars.push(new_half.get(i).copied().unwrap_or(0.0));
                                }
                                new_bars.truncate(total);

                                // Exponential smoothing: bars rise fast, fall slowly
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

                if should_close {
                    break;
                }
            }
        })
        .detach();
    }
}

impl Render for OverlayView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let w = self.overlay_config.width as f32;
        let h = self.overlay_config.height as f32;
        let bg_opacity = self.overlay_config.opacity;

        match self.mode {
            OverlayMode::Eq => render_eq(&self.eq_bars, w, h, bg_opacity, self.overlay_config.style),
            OverlayMode::Loading => render_loading(self.loading_tick, w, h, bg_opacity),
        }
    }
}

// ---------------------------------------------------------------------------
// EQ Rendering
// ---------------------------------------------------------------------------

fn render_eq(
    bars: &[f32],
    w: f32,
    h: f32,
    _bg_opacity: f32,
    style: OverlayStyle,
) -> gpui::Div {
    let bar_width: f32 = match style {
        OverlayStyle::Classic => 8.0,
        OverlayStyle::Mini => 12.0,
        OverlayStyle::None => 0.0,
    };
    let gap: f32 = match style {
        OverlayStyle::Classic => 4.0,
        OverlayStyle::Mini => 6.0,
        OverlayStyle::None => 0.0,
    };
    let max_bar_height = h - 24.0; // padding top + bottom

    div()
        .w(px(w))
        .h(px(h))
        .rounded(px(4.0))
        .bg(hsla(0.0, 0.0, 0.06, 0.97))
        .flex()
        .items_end()
        .justify_center()
        .gap(px(gap))
        .pb(px(10.0))
        .children(bars.iter().enumerate().map(|(i, &magnitude)| {
            let bar_h = (magnitude * max_bar_height).max(2.0);
            div()
                .id(SharedString::from(format!("bar-{i}")))
                .w(px(bar_width))
                .h(px(bar_h))
                .bg(hsla(0.0, 0.0, 0.78, 0.9))
        }))
}

// ---------------------------------------------------------------------------
// Loading Dots Rendering
// ---------------------------------------------------------------------------

fn render_loading(tick: usize, w: f32, h: f32, _bg_opacity: f32) -> gpui::Div {
    div()
        .w(px(w))
        .h(px(h))
        .rounded(px(4.0))
        .bg(hsla(0.0, 0.0, 0.06, 0.97))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .children((0..3).map(|i| {
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

    // Extract the most recent 1024 samples (power-of-two for FFT)
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
    drop(live); // Release lock before FFT computation

    use spectrum_analyzer::scaling::divide_by_N_sqrt;
    use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};

    let lo_hz: f32 = 80.0;
    let hi_hz: f32 = 8000.0;

    let spectrum = samples_fft_to_spectrum(
        &samples,
        sample_rate,
        FrequencyLimit::Range(lo_hz, hi_hz),
        Some(&divide_by_N_sqrt),
    );

    match spectrum {
        Ok(spectrum) => {
            let data = spectrum.data();
            if data.is_empty() {
                return vec![0.0; bar_count];
            }

            let log_lo = lo_hz.ln();
            let log_hi = hi_hz.ln();

            // For each bar, find the FFT bins whose frequency falls within
            // the bar's logarithmic frequency band.
            (0..bar_count)
                .map(|i| {
                    let t0 = i as f32 / bar_count as f32;
                    let t1 = (i + 1) as f32 / bar_count as f32;
                    let freq_lo = (log_lo + t0 * (log_hi - log_lo)).exp();
                    let freq_hi = (log_lo + t1 * (log_hi - log_lo)).exp();

                    let max_mag = data
                        .iter()
                        .filter(|(freq, _)| {
                            let f = freq.val();
                            f >= freq_lo && f < freq_hi
                        })
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
// OverlayController — manages the overlay window lifecycle from GPUI
// ---------------------------------------------------------------------------

pub struct OverlayController {
    shared: SharedState,
    window_handle: Option<WindowHandle<OverlayView>>,
}

impl OverlayController {
    pub fn new(shared: SharedState) -> Self {
        Self {
            shared,
            window_handle: None,
        }
    }

    pub fn start_polling(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let should_continue = this
                    .update(cx, |controller, cx| {
                        let phase = controller.shared.overlay_phase();
                        match phase {
                            OverlayPhase::Recording if controller.window_handle.is_none() => {
                                controller.open_overlay_window(cx);
                            }
                            OverlayPhase::Dismissed if controller.window_handle.is_some() => {
                                controller.close_overlay_window(cx);
                                controller
                                    .shared
                                    .set_overlay_phase(OverlayPhase::Hidden);
                            }
                            _ => {}
                        }
                        true
                    })
                    .unwrap_or_else(|_| {
                        eprintln!("[glide] overlay controller: entity released, stopping poll");
                        false
                    });
                if !should_continue {
                    break;
                }
            }
        })
        .detach();
    }

    fn open_overlay_window(&mut self, cx: &mut Context<Self>) {
        let config = self.shared.config();
        if config.overlay.style == OverlayStyle::None {
            return;
        }

        let (screen_w, screen_h) = crate::config::main_display_size();
        let ov = &config.overlay;
        let (x, y) = compute_overlay_position(ov, screen_w, screen_h);
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
            Ok(h) => self.window_handle = Some(h),
            Err(e) => eprintln!("[glide] failed to open overlay window: {e}"),
        }
    }

    fn close_overlay_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.window_handle.take() {
            let _ = handle.update(cx, |_view, window, _cx| {
                window.remove_window();
            });
        }
    }
}

fn compute_overlay_position(
    config: &OverlayConfig,
    screen_w: usize,
    screen_h: usize,
) -> (i32, i32) {
    let ow = config.width as i32;
    let oh = config.height as i32;
    let sw = screen_w as i32;
    let sh = screen_h as i32;

    match config.position.as_str() {
        "top-center" => ((sw - ow) / 2, 80),
        "center" => ((sw - ow) / 2, (sh - oh) / 2),
        // default: bottom-center
        _ => ((sw - ow) / 2, sh - oh - 80),
    }
}
