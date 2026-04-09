use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{fs};

use anyhow::{Context as _, Result};

// ---------------------------------------------------------------------------
// Objective-C runtime FFI
// ---------------------------------------------------------------------------

#[link(name = "AppKit", kind = "framework")]
#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {}

unsafe extern "C" {
    fn objc_getClass(name: *const u8) -> *mut c_void;
    fn sel_registerName(name: *const u8) -> *mut c_void;
    fn objc_msgSend(receiver: *mut c_void, sel: *mut c_void) -> *mut c_void;
}

type MsgSendPtr = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
type MsgSendUsize = unsafe extern "C" fn(*mut c_void, *mut c_void, usize, *mut c_void) -> *mut c_void;
type MsgSendLen = unsafe extern "C" fn(*mut c_void, *mut c_void) -> usize;

#[repr(C)]
#[derive(Copy, Clone)]
struct NSRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

type MsgSendRect = unsafe extern "C" fn(*mut c_void, *mut c_void) -> NSRect;

// ---------------------------------------------------------------------------
// Application listing
// ---------------------------------------------------------------------------

pub fn list_applications() -> Vec<String> {
    let mut apps: Vec<String> = std::fs::read_dir("/Applications")
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().map(|e| e == "app").unwrap_or(false) {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect();
    apps.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    apps
}

// ---------------------------------------------------------------------------
// App icon extraction
// ---------------------------------------------------------------------------

static ICON_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

fn icon_cache_dir() -> &'static Path {
    ICON_CACHE_DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join("glide-icons");
        let _ = fs::create_dir_all(&dir);
        dir
    })
}

pub fn app_icon_path(app_name: &str) -> Option<PathBuf> {
    let png_path = icon_cache_dir().join(format!("{app_name}.png"));
    if png_path.exists() {
        Some(png_path)
    } else {
        None
    }
}

pub fn accent_icon_path(accent: super::ColorAccent) -> Option<PathBuf> {
    let source_path = super::asset_path(accent.icns_asset());
    let png_path = icon_cache_dir().join(format!(
        "glide-app-icon-{}.png",
        accent.label().to_lowercase()
    ));
    let needs_refresh = match (fs::metadata(&source_path), fs::metadata(&png_path)) {
        (Ok(source), Ok(cached)) => match (source.modified(), cached.modified()) {
            (Ok(source_time), Ok(cached_time)) => source_time > cached_time,
            _ => true,
        },
        (Ok(_), Err(_)) => true,
        _ => false,
    };
    if (!png_path.exists() || needs_refresh) && extract_icon_file_to_png(&source_path, &png_path).is_err() {
        return None;
    }
    if png_path.exists() {
        Some(png_path)
    } else {
        None
    }
}

pub fn preload_app_icons() {
    std::thread::spawn(|| {
        let apps = list_applications();
        for app in &apps {
            let png_path = icon_cache_dir().join(format!("{app}.png"));
            if png_path.exists() {
                continue;
            }
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = extract_icon_to_png(app, &png_path);
            }));
        }
    });
}

fn extract_icon_to_png(app_name: &str, dest: &Path) -> Result<()> {
    let msg1: MsgSendPtr = unsafe { std::mem::transmute(objc_msgSend as *const ()) };

    unsafe {
        let workspace_class = objc_getClass(b"NSWorkspace\0".as_ptr());
        if workspace_class.is_null() {
            anyhow::bail!("NSWorkspace class not found");
        }
        let workspace = objc_msgSend(workspace_class, sel_registerName(b"sharedWorkspace\0".as_ptr()));
        if workspace.is_null() {
            anyhow::bail!("failed to get NSWorkspace");
        }

        let app_path = std::ffi::CString::new(format!("/Applications/{app_name}.app"))
            .context("invalid app name")?;
        let nsstring_class = objc_getClass(b"NSString\0".as_ptr());
        let ns_path = msg1(
            nsstring_class,
            sel_registerName(b"stringWithUTF8String:\0".as_ptr()),
            app_path.as_ptr() as *mut c_void,
        );
        if ns_path.is_null() {
            anyhow::bail!("failed to create NSString");
        }

        let icon = msg1(workspace, sel_registerName(b"iconForFile:\0".as_ptr()), ns_path);
        if icon.is_null() {
            anyhow::bail!("failed to get icon");
        }

        write_nsimage_png(icon, dest)
    }
}

fn extract_icon_file_to_png(source: &Path, dest: &Path) -> Result<()> {
    let source = source
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-utf8 icon path: {}", source.display()))?;

    let image = nsimage_from_path(source)?;
    write_nsimage_png(image, dest)
}

fn nsimage_from_path(path: &str) -> Result<*mut c_void> {
    unsafe {
        let msg_ptr: MsgSendPtr = std::mem::transmute(objc_msgSend as *const ());

        let ns_string_class = objc_getClass(b"NSString\0".as_ptr());
        if ns_string_class.is_null() {
            anyhow::bail!("NSString class not found");
        }
        let alloc = objc_msgSend(ns_string_class, sel_registerName(b"alloc\0".as_ptr()));
        type MsgSendInitString = unsafe extern "C" fn(*mut c_void, *mut c_void, *const u8, usize, usize) -> *mut c_void;
        let msg_init_str: MsgSendInitString = std::mem::transmute(objc_msgSend as *const ());
        let ns_path = msg_init_str(
            alloc,
            sel_registerName(b"initWithBytes:length:encoding:\0".as_ptr()),
            path.as_bytes().as_ptr(),
            path.len(),
            4, // NSUTF8StringEncoding
        );
        if ns_path.is_null() {
            anyhow::bail!("failed to create NSString path");
        }

        let ns_image_class = objc_getClass(b"NSImage\0".as_ptr());
        if ns_image_class.is_null() {
            anyhow::bail!("NSImage class not found");
        }
        let image_alloc = objc_msgSend(ns_image_class, sel_registerName(b"alloc\0".as_ptr()));
        let image = msg_ptr(
            image_alloc,
            sel_registerName(b"initWithContentsOfFile:\0".as_ptr()),
            ns_path,
        );
        if image.is_null() {
            anyhow::bail!("failed to load icon image");
        }

        Ok(image)
    }
}

fn write_nsimage_png(image: *mut c_void, dest: &Path) -> Result<()> {
    unsafe {
        let msg1: MsgSendPtr = std::mem::transmute(objc_msgSend as *const ());
        let msg_usize: MsgSendUsize = std::mem::transmute(objc_msgSend as *const ());
        let msg_len: MsgSendLen = std::mem::transmute(objc_msgSend as *const ());

        let tiff_data = objc_msgSend(image, sel_registerName(b"TIFFRepresentation\0".as_ptr()));
        if tiff_data.is_null() {
            anyhow::bail!("failed to get TIFF data");
        }

        let rep_class = objc_getClass(b"NSBitmapImageRep\0".as_ptr());
        if rep_class.is_null() {
            anyhow::bail!("NSBitmapImageRep class not found");
        }
        let rep = msg1(rep_class, sel_registerName(b"imageRepWithData:\0".as_ptr()), tiff_data);
        if rep.is_null() {
            anyhow::bail!("failed to create bitmap rep");
        }

        let dict_class = objc_getClass(b"NSDictionary\0".as_ptr());
        let empty_dict = objc_msgSend(dict_class, sel_registerName(b"dictionary\0".as_ptr()));
        let png_data = msg_usize(
            rep,
            sel_registerName(b"representationUsingType:properties:\0".as_ptr()),
            4, // NSBitmapImageFileTypePNG
            empty_dict,
        );
        if png_data.is_null() {
            anyhow::bail!("failed to create PNG data");
        }

        let bytes_ptr = objc_msgSend(png_data, sel_registerName(b"bytes\0".as_ptr())) as *const u8;
        let length = msg_len(png_data, sel_registerName(b"length\0".as_ptr()));
        if bytes_ptr.is_null() || length == 0 {
            anyhow::bail!("empty PNG data");
        }

        let bytes = std::slice::from_raw_parts(bytes_ptr, length);
        fs::write(dest, bytes)
            .with_context(|| format!("failed to write icon to {}", dest.display()))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Fuzzy matching
// ---------------------------------------------------------------------------

pub fn fuzzy_match(query: &str, candidate: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let query_lower = query.to_lowercase();
    let candidate_lower = candidate.to_lowercase();
    let mut score = 0i32;
    let mut qi = query_lower.chars().peekable();
    for (i, c) in candidate_lower.chars().enumerate() {
        if qi.peek() == Some(&c) {
            qi.next();
            score += 100 - i as i32;
        }
    }
    if qi.peek().is_none() {
        Some(score)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Frontmost app detection
// ---------------------------------------------------------------------------

pub fn frontmost_app_name() -> Option<String> {
    let msg1: MsgSendPtr = unsafe { std::mem::transmute(objc_msgSend as *const ()) };

    unsafe {
        let workspace_class = objc_getClass(b"NSWorkspace\0".as_ptr());
        if workspace_class.is_null() {
            return None;
        }
        let workspace = objc_msgSend(workspace_class, sel_registerName(b"sharedWorkspace\0".as_ptr()));
        if workspace.is_null() {
            return None;
        }

        let app = objc_msgSend(workspace, sel_registerName(b"frontmostApplication\0".as_ptr()));
        if app.is_null() {
            return None;
        }

        let ns_name = objc_msgSend(app, sel_registerName(b"localizedName\0".as_ptr()));
        if ns_name.is_null() {
            return None;
        }

        let cstr_ptr = msg1(ns_name, sel_registerName(b"UTF8String\0".as_ptr()), std::ptr::null_mut()) as *const i8;
        if cstr_ptr.is_null() {
            return None;
        }

        let name = std::ffi::CStr::from_ptr(cstr_ptr).to_string_lossy().into_owned();
        Some(name)
    }
}

// ---------------------------------------------------------------------------
// Screen size
// ---------------------------------------------------------------------------

unsafe extern "C" {
    fn CGMainDisplayID() -> u32;
    fn CGDisplayPixelsWide(display: u32) -> usize;
    fn CGDisplayPixelsHigh(display: u32) -> usize;
}

pub fn main_display_size() -> (usize, usize) {
    unsafe {
        let display = CGMainDisplayID();
        (CGDisplayPixelsWide(display), CGDisplayPixelsHigh(display))
    }
}

// ---------------------------------------------------------------------------
// Notch detection
// ---------------------------------------------------------------------------

pub fn notch_width() -> Option<u32> {
    unsafe {
        let ns_screen = objc_getClass(b"NSScreen\0".as_ptr());
        if ns_screen.is_null() {
            return None;
        }
        let screen = objc_msgSend(ns_screen, sel_registerName(b"mainScreen\0".as_ptr()));
        if screen.is_null() {
            return None;
        }

        let msg_rect: MsgSendRect = std::mem::transmute(objc_msgSend as *const ());

        let frame = msg_rect(screen, sel_registerName(b"frame\0".as_ptr()));
        let left_area = msg_rect(screen, sel_registerName(b"auxiliaryTopLeftArea\0".as_ptr()));
        let right_area = msg_rect(screen, sel_registerName(b"auxiliaryTopRightArea\0".as_ptr()));

        if left_area.w == 0.0 && right_area.w == 0.0 {
            return None;
        }

        let nw = frame.w - left_area.w - right_area.w;
        if nw > 0.0 { Some(nw as u32) } else { None }
    }
}

pub fn notch_dimensions() -> Option<(f64, f64)> {
    unsafe {
        let ns_screen = objc_getClass(b"NSScreen\0".as_ptr());
        if ns_screen.is_null() {
            return None;
        }
        let screen = objc_msgSend(ns_screen, sel_registerName(b"mainScreen\0".as_ptr()));
        if screen.is_null() {
            return None;
        }

        let msg_rect: MsgSendRect = std::mem::transmute(objc_msgSend as *const ());

        let frame = msg_rect(screen, sel_registerName(b"frame\0".as_ptr()));
        let left_area = msg_rect(screen, sel_registerName(b"auxiliaryTopLeftArea\0".as_ptr()));
        let right_area = msg_rect(screen, sel_registerName(b"auxiliaryTopRightArea\0".as_ptr()));

        if left_area.w == 0.0 && right_area.w == 0.0 {
            return None;
        }

        let nw = frame.w - left_area.w - right_area.w;
        let nh = left_area.h;
        if nw > 0.0 && nh > 0.0 { Some((nw, nh)) } else { None }
    }
}

pub fn set_dock_icon(accent: super::ColorAccent) {
    let path = super::asset_path(accent.icns_asset());
    if !path.exists() {
        eprintln!("[glide] icon not found: {}", path.display());
        return;
    }

    unsafe {
        let msg_ptr: MsgSendPtr = std::mem::transmute(objc_msgSend as *const ());
        let path_str = path.to_string_lossy();
        let image = match nsimage_from_path(&path_str) {
            Ok(image) => image,
            Err(err) => {
                eprintln!("[glide] failed to load dock icon {}: {err}", path.display());
                return;
            }
        };

        // NSApplication.shared.setApplicationIconImage:
        let ns_app_class = objc_getClass(b"NSApplication\0".as_ptr());
        if ns_app_class.is_null() {
            eprintln!("[glide] NSApplication class not found");
            return;
        }
        let shared_app = objc_msgSend(ns_app_class, sel_registerName(b"sharedApplication\0".as_ptr()));
        if shared_app.is_null() {
            eprintln!("[glide] failed to get NSApplication.shared");
            return;
        }
        msg_ptr(
            shared_app,
            sel_registerName(b"setApplicationIconImage:\0".as_ptr()),
            image,
        );

        // Force the Dock to refresh its cached icon tile
        let dock_tile = objc_msgSend(shared_app, sel_registerName(b"dockTile\0".as_ptr()));
        if !dock_tile.is_null() {
            objc_msgSend(dock_tile, sel_registerName(b"display\0".as_ptr()));
        }
    }
}
