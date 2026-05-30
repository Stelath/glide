use std::{
    error::Error,
    fmt,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::Instant,
};

use anyhow::{Context, Result};

use crate::benchmark::ProfileCollector;

use super::types::{HelperResponse, PersistentHelperRequest};

#[derive(Debug)]
struct PersistentTransportError {
    message: String,
}

impl PersistentTransportError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PersistentTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}

impl Error for PersistentTransportError {}

pub(super) struct PersistentHelperClient {
    helper: PathBuf,
    server: Option<PersistentHelperServer>,
}

struct PersistentHelperServer {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl PersistentHelperClient {
    pub(super) fn new(helper: PathBuf) -> Self {
        Self {
            helper,
            server: None,
        }
    }

    pub(super) fn request(
        &mut self,
        command: &str,
        input: &[u8],
        profile: &ProfileCollector,
    ) -> Result<HelperResponse> {
        let request = persistent_request_json(command, input)?;
        for attempt in 0..=1 {
            if self.server.is_none() {
                profile.measure_result("apple_helper_server_start", || self.start_server())?;
            } else {
                profile.record("apple_helper_server_reuse", std::time::Duration::ZERO);
            }

            let result = self
                .server
                .as_mut()
                .context("Apple persistent helper server was unavailable")?
                .send(command, &request, profile);

            match result {
                Ok(response) => return Ok(response),
                Err(error) if attempt == 0 && is_transport_error(error.as_ref()) => {
                    eprintln!(
                        "[glide] Apple helper: persistent server failed, restarting: {error:#}"
                    );
                    self.stop_server();
                }
                Err(error) => return Err(error),
            }
        }

        unreachable!("persistent helper retry loop should always return")
    }

    fn start_server(&mut self) -> Result<()> {
        let mut child = Command::new(&self.helper)
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| {
                format!(
                    "failed to start persistent Apple helper at {}",
                    self.helper.display()
                )
            })?;

        let stdin = child
            .stdin
            .take()
            .context("failed to open persistent Apple helper stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("failed to open persistent Apple helper stdout")?;

        eprintln!(
            "[glide] Apple helper: started persistent server at {}",
            self.helper.display()
        );
        self.server = Some(PersistentHelperServer {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        });
        Ok(())
    }

    fn stop_server(&mut self) {
        if let Some(mut server) = self.server.take() {
            let _ = server.child.kill();
            let _ = server.child.wait();
        }
    }
}

impl PersistentHelperServer {
    fn send(
        &mut self,
        command: &str,
        request: &[u8],
        profile: &ProfileCollector,
    ) -> Result<HelperResponse> {
        let round_trip_started = Instant::now();
        match command {
            "transcribe" => {
                profile.record_since_marker("stt_start", "stt_start_to_stt_helper_request_start");
                profile.record_since_marker(
                    "flow_release",
                    "flow_release_to_stt_helper_request_start",
                );
            }
            "cleanup" => {
                profile.record_since_marker("llm_start", "llm_start_to_llm_helper_request_start");
                profile.record_since_marker(
                    "flow_release",
                    "flow_release_to_llm_helper_request_start",
                );
                profile.record_since_marker(
                    "flow_stt_result",
                    "flow_stt_result_to_llm_helper_request_start",
                );
            }
            _ => {}
        }
        self.stdin
            .write_all(request)
            .map_err(|error| persistent_transport_error("write request", error))?;
        self.stdin
            .write_all(b"\n")
            .map_err(|error| persistent_transport_error("write request newline", error))?;
        self.stdin
            .flush()
            .map_err(|error| persistent_transport_error("flush request", error))?;

        let mut line = String::new();
        let bytes = self
            .stdout
            .read_line(&mut line)
            .map_err(|error| persistent_transport_error("read response", error))?;
        if bytes == 0 {
            return Err(
                PersistentTransportError::new("persistent Apple helper closed stdout").into(),
            );
        }

        profile.record("apple_helper_round_trip", round_trip_started.elapsed());
        decode_persistent_response(command, line.trim())
    }
}

pub(super) fn persistent_request_json(command: &str, input: &[u8]) -> Result<Vec<u8>> {
    let request = serde_json::from_slice(input)
        .with_context(|| format!("failed to encode persistent Apple helper {command} request"))?;
    serde_json::to_vec(&PersistentHelperRequest { command, request })
        .with_context(|| format!("failed to encode persistent Apple helper {command} envelope"))
}

pub(super) fn decode_persistent_response(command: &str, line: &str) -> Result<HelperResponse> {
    if line.is_empty() {
        anyhow::bail!("Apple helper returned an empty {command} response");
    }

    let response: HelperResponse = serde_json::from_str(line).with_context(|| {
        format!("failed to parse persistent Apple helper {command} response: {line}")
    })?;
    if !response.ok {
        anyhow::bail!(
            "{}",
            response
                .error
                .unwrap_or_else(|| format!("Apple helper returned an error for {command}"))
        );
    }
    Ok(response)
}

fn persistent_transport_error(action: &str, error: std::io::Error) -> anyhow::Error {
    PersistentTransportError::new(format!(
        "failed to {action} through persistent Apple helper: {error}"
    ))
    .into()
}

fn is_transport_error(error: &(dyn Error + 'static)) -> bool {
    error.is::<PersistentTransportError>()
}
