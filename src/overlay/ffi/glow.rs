use std::sync::{Arc, Mutex};

use super::objc::*;

pub(in crate::overlay) struct NotchGlowState {
    panel: *mut c_void,
}

unsafe impl Send for NotchGlowState {}
unsafe impl Sync for NotchGlowState {}

pub(in crate::overlay) fn create_notch_glow_panel(
    glow_rgb: Option<(f64, f64, f64)>,
) -> Option<Arc<Mutex<NotchGlowState>>> {
    let (notch_w, notch_h) = crate::platform::notch_dimensions()
        .unwrap_or((NOTCH_WIDTH_FALLBACK as f64, NOTCH_HEIGHT_FALLBACK));
    let panel_w = notch_w + 2.0 * GLOW_PADDING;
    let panel_h = notch_h + GLOW_PADDING;

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

        let ns_screen = objc_getClass(c"NSScreen".as_ptr());
        let main_screen = objc_msgSend(ns_screen, sel_registerName(c"mainScreen".as_ptr()));
        if main_screen.is_null() {
            return None;
        }
        let screen_frame = msg_rect(main_screen, sel_registerName(c"frame".as_ptr()));

        let x = (screen_frame.w - panel_w) / 2.0;
        let y_final = screen_frame.y + screen_frame.h - panel_h;
        let y_hidden = screen_frame.y + screen_frame.h;
        let ns_panel_class = objc_getClass(c"NSPanel".as_ptr());
        let panel = objc_msgSend(ns_panel_class, sel_registerName(c"alloc".as_ptr()));
        let content_rect = NSRect {
            x,
            y: y_hidden,
            w: panel_w,
            h: panel_h,
        };
        let panel = msg_init_rect(
            panel,
            sel_registerName(c"initWithContentRect:styleMask:backing:defer:".as_ptr()),
            content_rect,
            128,
            2,
            false,
        );
        if panel.is_null() {
            return None;
        }

        let clear_color = objc_msgSend(
            objc_getClass(c"NSColor".as_ptr()),
            sel_registerName(c"clearColor".as_ptr()),
        );
        msg_ptr(
            panel,
            sel_registerName(c"setBackgroundColor:".as_ptr()),
            clear_color,
        );
        msg_bool(panel, sel_registerName(c"setOpaque:".as_ptr()), false);
        msg_bool(panel, sel_registerName(c"setHasShadow:".as_ptr()), false);
        msg_i64(panel, sel_registerName(c"setLevel:".as_ptr()), 1000);
        msg_bool(
            panel,
            sel_registerName(c"setIgnoresMouseEvents:".as_ptr()),
            true,
        );
        msg_u64(
            panel,
            sel_registerName(c"setCollectionBehavior:".as_ptr()),
            1 << 0,
        );
        msg_bool(
            panel,
            sel_registerName(c"setHidesOnDeactivate:".as_ptr()),
            false,
        );

        let ns_view_class = objc_getClass(c"NSView".as_ptr());
        let content_view = objc_msgSend(ns_view_class, sel_registerName(c"alloc".as_ptr()));
        let content_view = objc_msgSend(content_view, sel_registerName(c"init".as_ptr()));
        let view_rect = NSRect {
            x: 0.0,
            y: 0.0,
            w: panel_w,
            h: panel_h,
        };
        msg_set_rect(
            content_view,
            sel_registerName(c"setFrame:".as_ptr()),
            view_rect,
            false,
        );
        msg_bool(
            content_view,
            sel_registerName(c"setWantsLayer:".as_ptr()),
            true,
        );
        let root_layer = objc_msgSend(content_view, sel_registerName(c"layer".as_ptr()));
        let clear_cg = objc_msgSend(clear_color, sel_registerName(c"CGColor".as_ptr()));
        msg_ptr(
            root_layer,
            sel_registerName(c"setBackgroundColor:".as_ptr()),
            clear_cg,
        );
        msg_ptr(
            panel,
            sel_registerName(c"setContentView:".as_ptr()),
            content_view,
        );
        objc_release(content_view);

        type MsgSendRGBA =
            unsafe extern "C" fn(*mut c_void, *mut c_void, f64, f64, f64, f64) -> *mut c_void;
        let msg_rgba: MsgSendRGBA = std::mem::transmute(objc_msgSend as *const ());

        let ns_color_class = objc_getClass(c"NSColor".as_ptr());
        let rgba_sel = sel_registerName(c"colorWithRed:green:blue:alpha:".as_ptr());

        let rainbow = glow_rgb.is_none();
        let (gr, gg, gb) = glow_rgb.unwrap_or((0.4, 0.7, 1.0));

        type MsgSendArrayObjs =
            unsafe extern "C" fn(*mut c_void, *mut c_void, *const *mut c_void, u64) -> *mut c_void;
        let msg_array: MsgSendArrayObjs = std::mem::transmute(objc_msgSend as *const ());
        let ns_array_class = objc_getClass(c"NSArray".as_ptr());
        let arr_sel = sel_registerName(c"arrayWithObjects:count:".as_ptr());
        let cg_color_sel = sel_registerName(c"CGColor".as_ptr());
        type MsgSendSetCGRect = unsafe extern "C" fn(*mut c_void, *mut c_void, NSRect);
        let msg_set_cg_rect: MsgSendSetCGRect = std::mem::transmute(objc_msgSend as *const ());
        type MsgSendSetCGPoint = unsafe extern "C" fn(*mut c_void, *mut c_void, f64, f64);
        let msg_set_point: MsgSendSetCGPoint = std::mem::transmute(objc_msgSend as *const ());
        #[repr(C)]
        struct CGAffineTransform {
            a: f64,
            b: f64,
            c: f64,
            d: f64,
            tx: f64,
            ty: f64,
        }
        type MsgSendSetAffineTransform =
            unsafe extern "C" fn(*mut c_void, *mut c_void, CGAffineTransform);
        let msg_set_affine_transform: MsgSendSetAffineTransform =
            std::mem::transmute(objc_msgSend as *const ());
        type MsgSendPtrPtr =
            unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
        let msg_ptr_ptr: MsgSendPtrPtr = std::mem::transmute(objc_msgSend as *const ());

        let ns_number = objc_getClass(c"NSNumber".as_ptr());
        let ca_anim_class = objc_getClass(c"CABasicAnimation".as_ptr());
        let anim_kp_sel = sel_registerName(c"animationWithKeyPath:".as_ptr());
        let timing = msg_ptr(
            objc_getClass(c"CAMediaTimingFunction".as_ptr()),
            sel_registerName(c"functionWithName:".as_ptr()),
            nsstring_cstr(c"easeInEaseOut"),
        );
        let linear_timing = msg_ptr(
            objc_getClass(c"CAMediaTimingFunction".as_ptr()),
            sel_registerName(c"functionWithName:".as_ptr()),
            nsstring_cstr(c"linear"),
        );

        let color_cg = |red: f64, green: f64, blue: f64, alpha: f64| {
            let color = msg_rgba(ns_color_class, rgba_sel, red, green, blue, alpha);
            objc_msgSend(color, cg_color_sel)
        };
        let number = |value: f64| {
            msg_f64(
                ns_number,
                sel_registerName(c"numberWithDouble:".as_ptr()),
                value,
            )
        };
        let add_basic_anim = |layer: *mut c_void,
                              key_path: &std::ffi::CStr,
                              from_value: f64,
                              to_value: f64,
                              duration: f64,
                              autoreverses: bool,
                              animation_key: &std::ffi::CStr| {
            let anim = msg_ptr(ca_anim_class, anim_kp_sel, nsstring_cstr(key_path));
            msg_ptr(
                anim,
                sel_registerName(c"setFromValue:".as_ptr()),
                number(from_value),
            );
            msg_ptr(
                anim,
                sel_registerName(c"setToValue:".as_ptr()),
                number(to_value),
            );
            msg_f64(anim, sel_registerName(c"setDuration:".as_ptr()), duration);
            msg_f32(
                anim,
                sel_registerName(c"setRepeatCount:".as_ptr()),
                f32::MAX,
            );
            msg_bool(
                anim,
                sel_registerName(c"setAutoreverses:".as_ptr()),
                autoreverses,
            );
            msg_ptr(
                anim,
                sel_registerName(c"setTimingFunction:".as_ptr()),
                if autoreverses { timing } else { linear_timing },
            );
            msg_ptr_ptr(
                layer,
                sel_registerName(c"addAnimation:forKey:".as_ptr()),
                anim,
                nsstring_cstr(animation_key),
            );
        };
        let set_gaussian_blur = |layer: *mut c_void, radius: f64| {
            let blur_filter = msg_ptr(
                objc_getClass(c"CIFilter".as_ptr()),
                sel_registerName(c"filterWithName:".as_ptr()),
                nsstring_cstr(c"CIGaussianBlur"),
            );
            if !blur_filter.is_null() {
                objc_msgSend(blur_filter, sel_registerName(c"setDefaults".as_ptr()));
                msg_ptr_ptr(
                    blur_filter,
                    sel_registerName(c"setValue:forKey:".as_ptr()),
                    number(radius),
                    nsstring_cstr(c"inputRadius"),
                );
                let filters = [blur_filter];
                let filters_arr = msg_array(
                    ns_array_class,
                    arr_sel,
                    filters.as_ptr(),
                    filters.len() as u64,
                );
                msg_ptr(
                    layer,
                    sel_registerName(c"setFilters:".as_ptr()),
                    filters_arr,
                );
            }
        };

        let brand_palette = [
            (0.94, 0.38, 0.23),
            (0.49, 0.42, 0.77),
            (0.29, 0.56, 0.83),
            (0.94, 0.38, 0.23),
        ];
        let mix_with_accent = |(br, bg, bb): (f64, f64, f64)| {
            if rainbow {
                (br, bg, bb)
            } else {
                (
                    gr * 0.58 + br * 0.42,
                    gg * 0.58 + bg * 0.42,
                    gb * 0.58 + bb * 0.42,
                )
            }
        };
        let aura_palette: Vec<(f64, f64, f64)> =
            brand_palette.iter().copied().map(mix_with_accent).collect();

        let grad_class = objc_getClass(c"CAGradientLayer".as_ptr());
        let layer_class = objc_getClass(c"CALayer".as_ptr());
        let aura_center_x = panel_w / 2.0;
        let aura_center_y = GLOW_PADDING + GLOW_AURA_NOTCH_OFFSET;
        let aura_w = GLOW_AURA_SIZE;
        let aura_h = GLOW_AURA_SIZE;
        let aura_outer_w =
            aura_w * GLOW_AURA_SCALE_X * GLOW_BREATHE_MAX_SCALE + 2.0 * GLOW_BLUR_RADIUS;
        let aura_outer_h =
            aura_h * GLOW_AURA_SCALE_Y * GLOW_BREATHE_MAX_SCALE + 2.0 * GLOW_BLUR_RADIUS;
        let aura_rect = NSRect {
            x: aura_center_x - aura_outer_w / 2.0,
            y: aura_center_y - aura_outer_h / 2.0,
            w: aura_outer_w,
            h: aura_outer_h,
        };

        let aura_container = objc_msgSend(layer_class, sel_registerName(c"new".as_ptr()));
        msg_set_cg_rect(
            aura_container,
            sel_registerName(c"setFrame:".as_ptr()),
            aura_rect,
        );
        msg_bool(
            aura_container,
            sel_registerName(c"setMasksToBounds:".as_ptr()),
            false,
        );
        msg_f32(
            aura_container,
            sel_registerName(c"setOpacity:".as_ptr()),
            GLOW_AURA_OPACITY as f32,
        );
        set_gaussian_blur(aura_container, GLOW_BLUR_RADIUS);

        msg_ptr(
            root_layer,
            sel_registerName(c"addSublayer:".as_ptr()),
            aura_container,
        );

        let flare_outer_w = aura_w * 2.72 * GLOW_BREATHE_MAX_SCALE + 2.0 * GLOW_FLARE_BLUR_RADIUS;
        let flare_outer_h = aura_h * 1.08 * GLOW_BREATHE_MAX_SCALE + 2.0 * GLOW_FLARE_BLUR_RADIUS;
        let flare_rect = NSRect {
            x: aura_center_x - flare_outer_w / 2.0,
            y: aura_center_y - flare_outer_h / 2.0,
            w: flare_outer_w,
            h: flare_outer_h,
        };
        let flare_container = objc_msgSend(layer_class, sel_registerName(c"new".as_ptr()));
        msg_set_cg_rect(
            flare_container,
            sel_registerName(c"setFrame:".as_ptr()),
            flare_rect,
        );
        msg_bool(
            flare_container,
            sel_registerName(c"setMasksToBounds:".as_ptr()),
            false,
        );
        msg_f32(
            flare_container,
            sel_registerName(c"setOpacity:".as_ptr()),
            GLOW_FLARE_OPACITY as f32,
        );
        set_gaussian_blur(flare_container, GLOW_FLARE_BLUR_RADIUS);
        msg_ptr(
            root_layer,
            sel_registerName(c"addSublayer:".as_ptr()),
            flare_container,
        );

        let flare_breathe = objc_msgSend(layer_class, sel_registerName(c"new".as_ptr()));
        let flare_breathe_rect = NSRect {
            x: 0.0,
            y: 0.0,
            w: flare_outer_w,
            h: flare_outer_h,
        };
        msg_set_cg_rect(
            flare_breathe,
            sel_registerName(c"setFrame:".as_ptr()),
            flare_breathe_rect,
        );
        msg_ptr(
            flare_container,
            sel_registerName(c"addSublayer:".as_ptr()),
            flare_breathe,
        );

        let flare_center_x = flare_outer_w / 2.0;
        let flare_center_y = flare_outer_h / 2.0;
        let add_flare_streak = |width: f64,
                                height: f64,
                                y_offset: f64,
                                rotation: f64,
                                color: (f64, f64, f64),
                                alpha: f64,
                                opacity: f32| {
            let streak = objc_msgSend(grad_class, sel_registerName(c"new".as_ptr()));
            let streak_rect = NSRect {
                x: flare_center_x - width / 2.0,
                y: flare_center_y - height / 2.0 + y_offset,
                w: width,
                h: height,
            };
            msg_set_cg_rect(streak, sel_registerName(c"setFrame:".as_ptr()), streak_rect);
            msg_set_point(
                streak,
                sel_registerName(c"setStartPoint:".as_ptr()),
                0.0,
                0.5,
            );
            msg_set_point(streak, sel_registerName(c"setEndPoint:".as_ptr()), 1.0, 0.5);
            msg_f64(
                streak,
                sel_registerName(c"setCornerRadius:".as_ptr()),
                height / 2.0,
            );
            msg_bool(
                streak,
                sel_registerName(c"setMasksToBounds:".as_ptr()),
                true,
            );
            msg_f32(streak, sel_registerName(c"setOpacity:".as_ptr()), opacity);

            let (cr, cg_c, cb) = color;
            let streak_colors = [
                color_cg(cr, cg_c, cb, 0.0),
                color_cg(cr, cg_c, cb, alpha * 0.36),
                color_cg(1.0, 0.96, 0.88, alpha),
                color_cg(cr, cg_c, cb, alpha * 0.36),
                color_cg(cr, cg_c, cb, 0.0),
            ];
            let streak_colors_arr = msg_array(
                ns_array_class,
                arr_sel,
                streak_colors.as_ptr(),
                streak_colors.len() as u64,
            );
            msg_ptr(
                streak,
                sel_registerName(c"setColors:".as_ptr()),
                streak_colors_arr,
            );

            let streak_locations = [
                number(0.0),
                number(0.38),
                number(0.5),
                number(0.62),
                number(1.0),
            ];
            let streak_locations_arr = msg_array(
                ns_array_class,
                arr_sel,
                streak_locations.as_ptr(),
                streak_locations.len() as u64,
            );
            msg_ptr(
                streak,
                sel_registerName(c"setLocations:".as_ptr()),
                streak_locations_arr,
            );

            let rotation_cos = rotation.cos();
            let rotation_sin = rotation.sin();
            msg_set_affine_transform(
                streak,
                sel_registerName(c"setAffineTransform:".as_ptr()),
                CGAffineTransform {
                    a: rotation_cos,
                    b: rotation_sin,
                    c: -rotation_sin,
                    d: rotation_cos,
                    tx: 0.0,
                    ty: 0.0,
                },
            );
            msg_ptr(
                flare_breathe,
                sel_registerName(c"addSublayer:".as_ptr()),
                streak,
            );
            objc_release(streak);
        };
        add_flare_streak(aura_w * 2.55, 10.0, 0.0, 0.0, aura_palette[0], 0.86, 0.95);
        add_flare_streak(aura_w * 2.05, 6.0, 11.0, 0.08, aura_palette[2], 0.62, 0.82);
        add_flare_streak(aura_w * 1.8, 5.0, -10.0, -0.13, aura_palette[1], 0.56, 0.76);
        add_flare_streak(aura_w * 1.35, 4.0, 19.0, -0.2, aura_palette[0], 0.42, 0.56);

        let aura_scaled = objc_msgSend(layer_class, sel_registerName(c"new".as_ptr()));
        let scaled_rect = NSRect {
            x: aura_outer_w / 2.0 - aura_w / 2.0,
            y: aura_outer_h / 2.0 - aura_h / 2.0,
            w: aura_w,
            h: aura_h,
        };
        msg_set_cg_rect(
            aura_scaled,
            sel_registerName(c"setFrame:".as_ptr()),
            scaled_rect,
        );
        msg_bool(
            aura_scaled,
            sel_registerName(c"setMasksToBounds:".as_ptr()),
            false,
        );
        msg_set_affine_transform(
            aura_scaled,
            sel_registerName(c"setAffineTransform:".as_ptr()),
            CGAffineTransform {
                a: GLOW_AURA_SCALE_X,
                b: 0.0,
                c: 0.0,
                d: GLOW_AURA_SCALE_Y,
                tx: 0.0,
                ty: 0.0,
            },
        );
        msg_ptr(
            aura_container,
            sel_registerName(c"addSublayer:".as_ptr()),
            aura_scaled,
        );

        let aura_breathe = objc_msgSend(layer_class, sel_registerName(c"new".as_ptr()));
        let breathe_rect = NSRect {
            x: 0.0,
            y: 0.0,
            w: aura_w,
            h: aura_h,
        };
        msg_set_cg_rect(
            aura_breathe,
            sel_registerName(c"setFrame:".as_ptr()),
            breathe_rect,
        );
        msg_ptr(
            aura_scaled,
            sel_registerName(c"addSublayer:".as_ptr()),
            aura_breathe,
        );

        let aura_glow = objc_msgSend(grad_class, sel_registerName(c"new".as_ptr()));
        let local_aura_rect = NSRect {
            x: 0.0,
            y: 0.0,
            w: aura_w,
            h: aura_h,
        };
        msg_set_cg_rect(
            aura_glow,
            sel_registerName(c"setFrame:".as_ptr()),
            local_aura_rect,
        );
        msg_ptr(
            aura_glow,
            sel_registerName(c"setType:".as_ptr()),
            nsstring_cstr(c"conic"),
        );
        msg_set_point(
            aura_glow,
            sel_registerName(c"setStartPoint:".as_ptr()),
            0.5,
            0.5,
        );
        msg_set_point(
            aura_glow,
            sel_registerName(c"setEndPoint:".as_ptr()),
            1.0,
            1.0,
        );
        msg_f64(
            aura_glow,
            sel_registerName(c"setCornerRadius:".as_ptr()),
            aura_h / 2.0,
        );
        msg_bool(
            aura_glow,
            sel_registerName(c"setMasksToBounds:".as_ptr()),
            true,
        );
        let aura_colors: Vec<*mut c_void> = aura_palette
            .iter()
            .map(|&(cr, cg_c, cb)| color_cg(cr, cg_c, cb, 0.88))
            .collect();
        let aura_colors_arr = msg_array(
            ns_array_class,
            arr_sel,
            aura_colors.as_ptr(),
            aura_colors.len() as u64,
        );
        msg_ptr(
            aura_glow,
            sel_registerName(c"setColors:".as_ptr()),
            aura_colors_arr,
        );

        let aura_mask = objc_msgSend(grad_class, sel_registerName(c"new".as_ptr()));
        msg_set_cg_rect(
            aura_mask,
            sel_registerName(c"setFrame:".as_ptr()),
            local_aura_rect,
        );
        msg_ptr(
            aura_mask,
            sel_registerName(c"setType:".as_ptr()),
            nsstring_cstr(c"radial"),
        );
        msg_set_point(
            aura_mask,
            sel_registerName(c"setStartPoint:".as_ptr()),
            0.5,
            0.5,
        );
        msg_set_point(
            aura_mask,
            sel_registerName(c"setEndPoint:".as_ptr()),
            1.0,
            1.0,
        );
        let mask_colors = [
            color_cg(0.0, 0.0, 0.0, 1.0),
            color_cg(0.0, 0.0, 0.0, 0.84),
            color_cg(0.0, 0.0, 0.0, 0.0),
        ];
        let mask_colors_arr = msg_array(
            ns_array_class,
            arr_sel,
            mask_colors.as_ptr(),
            mask_colors.len() as u64,
        );
        msg_ptr(
            aura_mask,
            sel_registerName(c"setColors:".as_ptr()),
            mask_colors_arr,
        );
        let mask_locations = [number(0.0), number(0.64), number(1.0)];
        let mask_locations_arr = msg_array(
            ns_array_class,
            arr_sel,
            mask_locations.as_ptr(),
            mask_locations.len() as u64,
        );
        msg_ptr(
            aura_mask,
            sel_registerName(c"setLocations:".as_ptr()),
            mask_locations_arr,
        );
        msg_ptr(aura_glow, sel_registerName(c"setMask:".as_ptr()), aura_mask);

        msg_ptr(
            aura_breathe,
            sel_registerName(c"addSublayer:".as_ptr()),
            aura_glow,
        );
        add_basic_anim(
            aura_container,
            c"opacity",
            GLOW_AURA_OPACITY * 0.55,
            GLOW_AURA_OPACITY,
            GLOW_BREATHE_DURATION,
            true,
            c"containerOpacity",
        );
        add_basic_anim(
            flare_container,
            c"opacity",
            GLOW_FLARE_OPACITY * 0.48,
            GLOW_FLARE_OPACITY,
            GLOW_BREATHE_DURATION,
            true,
            c"flareOpacity",
        );
        add_basic_anim(
            aura_breathe,
            c"transform.scale",
            GLOW_BREATHE_MIN_SCALE,
            GLOW_BREATHE_MAX_SCALE,
            GLOW_BREATHE_DURATION,
            true,
            c"breatheAura",
        );
        add_basic_anim(
            flare_breathe,
            c"transform.scale",
            GLOW_BREATHE_MIN_SCALE,
            GLOW_BREATHE_MAX_SCALE,
            GLOW_BREATHE_DURATION,
            true,
            c"breatheFlare",
        );
        add_basic_anim(
            aura_glow,
            c"transform.rotation.z",
            0.0,
            std::f64::consts::TAU,
            GLOW_SPIN_DURATION,
            false,
            c"spinAura",
        );
        objc_release(aura_mask);
        objc_release(aura_glow);
        objc_release(aura_breathe);
        objc_release(aura_scaled);
        objc_release(flare_breathe);
        objc_release(flare_container);
        objc_release(aura_container);

        objc_msgSend(panel, sel_registerName(c"orderFrontRegardless".as_ptr()));
        let ns_anim = objc_getClass(c"NSAnimationContext".as_ptr());
        objc_msgSend(ns_anim, sel_registerName(c"beginGrouping".as_ptr()));
        let current_ctx = objc_msgSend(ns_anim, sel_registerName(c"currentContext".as_ptr()));
        msg_f64(current_ctx, sel_registerName(c"setDuration:".as_ptr()), 0.2);
        type MsgSend4F =
            unsafe extern "C" fn(*mut c_void, *mut c_void, f32, f32, f32, f32) -> *mut c_void;
        let msg_4f: MsgSend4F = std::mem::transmute(objc_msgSend as *const ());
        let slide_timing = msg_4f(
            objc_getClass(c"CAMediaTimingFunction".as_ptr()),
            sel_registerName(c"functionWithControlPoints::::".as_ptr()),
            0.0,
            0.0,
            0.2,
            1.0,
        );
        msg_ptr(
            current_ctx,
            sel_registerName(c"setTimingFunction:".as_ptr()),
            slide_timing,
        );
        let animator = objc_msgSend(panel, sel_registerName(c"animator".as_ptr()));
        let final_rect = NSRect {
            x,
            y: y_final,
            w: panel_w,
            h: panel_h,
        };
        msg_set_rect(
            animator,
            sel_registerName(c"setFrame:display:".as_ptr()),
            final_rect,
            true,
        );
        objc_msgSend(ns_anim, sel_registerName(c"endGrouping".as_ptr()));

        Some(Arc::new(Mutex::new(NotchGlowState { panel })))
    }
}

pub(in crate::overlay) fn close_notch_glow_panel(state: &NotchGlowState) {
    unsafe {
        objc_msgSend(state.panel, sel_registerName(c"orderOut:".as_ptr()));
        objc_release(state.panel);
    }
}
