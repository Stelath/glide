/// Returns the glide-core library version as a C string.
/// Scaffolding proof-of-concept for Rust-Swift linkage.
#[unsafe(no_mangle)]
pub extern "C" fn glide_core_version() -> *const std::ffi::c_char {
    c"0.1.0".as_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_null() {
        let ptr = glide_core_version();
        assert!(!ptr.is_null());
        let s = unsafe { std::ffi::CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "0.1.0");
    }
}
