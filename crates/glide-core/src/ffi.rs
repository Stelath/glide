use std::ffi::{CStr, CString};
use std::os::raw::c_char;

pub fn json_ok<T: serde::Serialize>(data: T) -> *mut c_char {
    let json = serde_json::json!({"ok": true, "data": data});
    to_c_string(&json.to_string())
}

pub fn json_err(msg: &str) -> *mut c_char {
    let json = serde_json::json!({"ok": false, "error": msg});
    to_c_string(&json.to_string())
}

pub fn to_c_string(s: &str) -> *mut c_char {
    CString::new(s).unwrap_or_default().into_raw()
}

pub unsafe fn free_c_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = unsafe { CString::from_raw(ptr) };
    }
}

pub unsafe fn c_str_to_str<'a>(ptr: *const c_char) -> Result<&'a str, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }

    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map_err(|e| format!("invalid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_helpers_round_trip() {
        let ptr = json_ok("hello");
        let value = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string();
        assert_eq!(value, r#"{"data":"hello","ok":true}"#);
        unsafe { free_c_string(ptr) };
    }
}
