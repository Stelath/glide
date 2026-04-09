mod ffi;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::{
    AppContext, Bounds, Context, Entity, Global, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, Styled, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, div, hsla, point, px, size,
};

use crate::app::{OverlayPhase, SharedState};
use crate::config::{OverlayConfig, OverlayPosition, OverlayStyle};

use ffi::*;

#[allow(dead_code)]
pub struct OverlayHandle(pub Entity<OverlayController>);
impl Global for OverlayHandle {}

// ---------------------------------------------------------------------------
// GPUI overlay view (floating mode)
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
    bar_color: (f32, f32, f32, f32),
}

impl OverlayView {
    fn new(
        shared: SharedState,
        overlay_config: OverlayConfig,
        bar_color: (f32, f32, f32, f32),
    ) -> Self {
        let bar_count = match overlay_config.style {
            OverlayStyle::Classic => 16,
            OverlayStyle::Glow | OverlayStyle::None => 0,
        };
        Self {
            shared,
            mode: OverlayMode::Eq,
            eq_bars: vec![0.0; bar_count],
            loading_tick: 0,
            overlay_config,
            bar_color,
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
        let bc = self.bar_color;
        match self.mode {
            OverlayMode::Eq => render_eq(&self.eq_bars, w, h, self.overlay_config.style, bc),
            OverlayMode::Loading => render_loading(self.loading_tick, w, h, bc),
        }
    }
}

fn render_eq(
    bars: &[f32],
    w: f32,
    h: f32,
    style: OverlayStyle,
    bar_color: (f32, f32, f32, f32),
) -> gpui::Div {
    let (bar_width, gap) = match style {
        OverlayStyle::Classic => (8.0f32, 4.0f32),
        OverlayStyle::Glow | OverlayStyle::None => (0.0, 0.0),
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
            let (bh, bs, bl, ba) = bar_color;
            div()
                .id(SharedString::from(format!("bar-{i}")))
                .w(px(bar_width))
                .h(px(bar_h))
                .bg(hsla(bh, bs, bl, ba))
        }))
}

fn render_loading(tick: usize, w: f32, h: f32, bar_color: (f32, f32, f32, f32)) -> gpui::Div {
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
            let (bh, bs, bl, _) = bar_color;
            div()
                .id(SharedString::from(format!("dot-{i}")))
                .w(px(6.0))
                .h(px(6.0))
                .rounded(px(3.0))
                .bg(hsla(bh, bs, bl, dot_opacity))
        }))
}

// ---------------------------------------------------------------------------
// FFT computation
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
    use spectrum_analyzer::{FrequencyLimit, samples_fft_to_spectrum};
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
// Overlay controller
// ---------------------------------------------------------------------------

enum ActiveOverlay {
    Gpui(WindowHandle<OverlayView>),
    Notch(Arc<Mutex<NotchPanelState>>),
    NotchGlow(Arc<Mutex<NotchGlowState>>),
}

pub struct OverlayController {
    shared: SharedState,
    active: Option<ActiveOverlay>,
}

impl OverlayController {
    pub fn new(shared: SharedState) -> Self {
        Self {
            shared,
            active: None,
        }
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
                if !should_continue {
                    break;
                }
            }
        })
        .detach();
    }

    fn open_overlay(&mut self, cx: &mut Context<Self>) {
        let config = self.shared.config();
        if config.overlay.style == OverlayStyle::None {
            return;
        }

        if config.overlay.style == OverlayStyle::Glow {
            if let Some(state) = create_notch_glow_panel(config.app.accent.glow_rgb()) {
                self.active = Some(ActiveOverlay::NotchGlow(state));
            } else {
                eprintln!("[glide] glow overlay: notch not detected, cannot create glow panel");
            }
            return;
        }

        match config.overlay.position {
            OverlayPosition::Notch => {
                if let Some(state) = create_notch_panel(config.app.accent.bar_rgba()) {
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
                let bar_color = config.app.accent.bar_hsla();

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
                            OverlayView::new(shared, overlay_config, bar_color)
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
            Some(ActiveOverlay::NotchGlow(state)) => {
                let s = state.lock().unwrap();
                close_notch_glow_panel(&s);
            }
            None => {}
        }
    }
}
