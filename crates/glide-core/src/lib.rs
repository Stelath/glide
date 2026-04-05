mod ffi;
mod llm;
mod models;
mod stt;

use std::ffi::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn glide_core_version() -> *const c_char {
    c"0.1.0".as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn glide_core_transcribe(
    audio_bytes: *const u8,
    audio_len: u32,
    config_json: *const c_char,
) -> *mut c_char {
    let result = std::panic::catch_unwind(|| -> Result<String, String> {
        if audio_len == 0 {
            return Err("audio buffer is empty".to_string());
        }
        if audio_bytes.is_null() {
            return Err("audio buffer pointer is null".to_string());
        }

        let config_str = unsafe { ffi::c_str_to_str(config_json) }?;
        let config: stt::TranscribeConfig =
            serde_json::from_str(config_str).map_err(|e| format!("invalid config: {e}"))?;
        let audio = unsafe { std::slice::from_raw_parts(audio_bytes, audio_len as usize) };

        let runtime =
            tokio::runtime::Runtime::new().map_err(|e| format!("failed to create runtime: {e}"))?;
        runtime
            .block_on(stt::transcribe(audio, &config))
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(Ok(text)) => ffi::json_ok(text),
        Ok(Err(error)) => ffi::json_err(&error),
        Err(_) => ffi::json_err("internal panic in glide_core_transcribe"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glide_core_cleanup(
    raw_text: *const c_char,
    config_json: *const c_char,
) -> *mut c_char {
    let result = std::panic::catch_unwind(|| -> Result<String, String> {
        let raw_text = unsafe { ffi::c_str_to_str(raw_text) }?;
        let config_str = unsafe { ffi::c_str_to_str(config_json) }?;
        let config: llm::CleanupConfig =
            serde_json::from_str(config_str).map_err(|e| format!("invalid config: {e}"))?;

        let runtime =
            tokio::runtime::Runtime::new().map_err(|e| format!("failed to create runtime: {e}"))?;
        runtime
            .block_on(llm::cleanup(raw_text, &config))
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(Ok(text)) => ffi::json_ok(text),
        Ok(Err(error)) => ffi::json_err(&error),
        Err(_) => ffi::json_err("internal panic in glide_core_cleanup"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glide_core_fetch_models(config_json: *const c_char) -> *mut c_char {
    let result = std::panic::catch_unwind(|| -> Result<models::ModelsResult, String> {
        let config_str = unsafe { ffi::c_str_to_str(config_json) }?;
        let config: models::FetchModelsConfig =
            serde_json::from_str(config_str).map_err(|e| format!("invalid config: {e}"))?;

        let runtime =
            tokio::runtime::Runtime::new().map_err(|e| format!("failed to create runtime: {e}"))?;
        runtime
            .block_on(models::fetch_models(&config))
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(Ok(data)) => ffi::json_ok(data),
        Ok(Err(error)) => ffi::json_err(&error),
        Err(_) => ffi::json_err("internal panic in glide_core_fetch_models"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glide_core_free_string(s: *mut c_char) {
    unsafe { ffi::free_c_string(s) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, CString};

    #[test]
    fn version_is_not_null() {
        let ptr = glide_core_version();
        assert!(!ptr.is_null());
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "0.1.0");
    }

    #[test]
    fn cleanup_reports_invalid_config() {
        let text = CString::new("hello").unwrap();
        let config = CString::new("{bad json").unwrap();

        let ptr = glide_core_cleanup(text.as_ptr(), config.as_ptr());
        let response = take_string(ptr);

        assert!(response.contains("\"ok\":false"));
        assert!(response.contains("invalid config"));
    }

    #[test]
    fn transcribe_rejects_empty_audio() {
        let config = CString::new(
            r#"{"provider":"openai","model":"whisper-1","api_key":"key","base_url":"https://example.com/v1"}"#,
        )
        .unwrap();

        let ptr = glide_core_transcribe(std::ptr::null(), 0, config.as_ptr());
        let response = take_string(ptr);

        assert!(response.contains("\"ok\":false"));
        assert!(response.contains("audio buffer is empty"));
    }

    fn take_string(ptr: *mut c_char) -> String {
        assert!(!ptr.is_null());
        let value = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string();
        glide_core_free_string(ptr);
        value
    }
}
