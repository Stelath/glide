use std::{
    io::Write,
    path::PathBuf,
    process::{Command, ExitStatus, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};

use super::types::HelperResponse;

pub(super) fn run_helper(command: &str, input: Option<&[u8]>) -> Result<HelperResponse> {
    let helper = helper_path()?;
    let mut child = Command::new(&helper)
        .arg(command)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start Apple helper at {}", helper.display()))?;

    if let Some(input) = input {
        let stdin = child
            .stdin
            .as_mut()
            .context("failed to open Apple helper stdin")?;
        stdin
            .write_all(input)
            .context("failed to write Apple helper request")?;
    }

    let output = child
        .wait_with_output()
        .context("failed to wait for Apple helper")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    decode_helper_response(command, &output.status, stdout.trim(), stderr.trim())
}

pub(super) fn decode_helper_response(
    command: &str,
    status: &ExitStatus,
    stdout: &str,
    stderr: &str,
) -> Result<HelperResponse> {
    if stdout.is_empty() {
        anyhow::bail!("{}", helper_failure_message(command, status, stderr));
    }

    let response: HelperResponse = serde_json::from_str(stdout).with_context(|| {
        format!(
            "failed to parse Apple helper response; status: {}, stderr: {}",
            status, stderr
        )
    })?;

    if !status.success() || !response.ok {
        anyhow::bail!(
            "{}",
            response
                .error
                .unwrap_or_else(|| helper_failure_message(command, status, stderr))
        );
    }

    Ok(response)
}

pub(crate) fn helper_failure_message(command: &str, status: &ExitStatus, stderr: &str) -> String {
    if command == "foundation-models" && helper_was_fatal_signal(status) {
        return "Apple Foundation model availability check failed: BackgroundAssets validation failed."
            .to_string();
    }

    if command == "capabilities" && helper_was_fatal_signal(status) {
        return "Apple local provider capabilities unavailable: BackgroundAssets validation failed."
            .to_string();
    }

    let mut message = format!(
        "Apple helper failed while running {command}: {}",
        helper_exit_description(status)
    );
    if !stderr.trim().is_empty() {
        message.push_str("; ");
        message.push_str(stderr.trim());
    }
    if helper_was_fatal_signal(status) {
        message.push_str(
            ". Apple local model APIs can abort when BackgroundAssets cannot validate the running app bundle. Use the signed app bundle path; if this persists, the Apple API may require additional provisioning.",
        );
    }
    message
}

fn helper_exit_description(status: &ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            let name = signal_name(signal)
                .map(|name| format!(" ({name})"))
                .unwrap_or_default();
            return format!("signal {signal}{name}");
        }
    }

    format!("status {status}")
}

fn helper_was_fatal_signal(status: &ExitStatus) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        matches!(status.signal(), Some(5 | 6))
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        false
    }
}

fn signal_name(signal: i32) -> Option<&'static str> {
    match signal {
        5 => Some("SIGTRAP"),
        6 => Some("SIGABRT"),
        9 => Some("SIGKILL"),
        15 => Some("SIGTERM"),
        _ => None,
    }
}

pub(crate) fn helper_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("failed to locate current executable")?;
    if let Some(parent) = exe.parent() {
        if let Some(contents_dir) = parent.parent() {
            let nested = contents_dir
                .join("Helpers")
                .join("GlideAppleHelper.app")
                .join("Contents")
                .join("MacOS")
                .join("GlideAppleHelper");
            if nested.is_file() {
                return Ok(nested);
            }
        }

        let bundled = parent.join("GlideAppleHelper");
        if bundled.is_file() {
            return Ok(bundled);
        }
    }

    if let Ok(path) = std::env::var("GLIDE_BENCH_APPLE_HELPER_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
    }

    if let Some(path) = option_env!("GLIDE_APPLE_HELPER_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
    }

    anyhow::bail!("Apple helper is not available in this build")
}

pub(super) fn write_temp_audio(audio: &[u8]) -> Result<PathBuf> {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("glide-apple-speech-{suffix}.wav"));
    std::fs::write(&path, audio)
        .with_context(|| format!("failed to write temporary audio to {}", path.display()))?;
    Ok(path)
}
