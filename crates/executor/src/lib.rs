use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use reprorun_config::CommandSpec;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("shell command mode is disabled")]
    ShellDisabled,
    #[error("empty argv command")]
    EmptyCommand,
    #[error("failed to spawn command: {0}")]
    Spawn(std::io::Error),
    #[error("execution I/O failed: {0}")]
    Io(std::io::Error),
}

#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    pub command: CommandSpec,
    pub working_dir: Option<std::path::PathBuf>,
    pub env: BTreeMap<String, String>,
    pub stdin: Option<Vec<u8>>,
    pub timeout_ms: Option<u64>,
    pub output_max_bytes: usize,
    pub stream_output: bool,
    pub allow_shell: bool,
    pub seed: Option<u64>,
    pub time_epoch: Option<i64>,
}

impl Default for ExecutionRequest {
    fn default() -> Self {
        Self {
            command: CommandSpec::Argv(Vec::new()),
            working_dir: None,
            env: BTreeMap::new(),
            stdin: None,
            timeout_ms: None,
            output_max_bytes: 10 * 1024 * 1024,
            stream_output: true,
            allow_shell: false,
            seed: None,
            time_epoch: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitReason {
    Exited,
    TimeoutKilled,
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    pub exit_reason: ExitReason,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

pub fn execute(request: &ExecutionRequest) -> Result<ExecutionResult, ExecutorError> {
    let started = Instant::now();
    let mut command = build_command(request)?;

    let mut child = command.spawn().map_err(ExecutorError::Spawn)?;
    if let Some(stdin) = &request.stdin {
        if let Some(mut handle) = child.stdin.take() {
            handle.write_all(stdin).map_err(ExecutorError::Io)?;
        }
    } else {
        let _ = child.stdin.take();
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ExecutorError::Io(std::io::Error::other("stdout not captured")))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ExecutorError::Io(std::io::Error::other("stderr not captured")))?;

    let (stdout_tx, stdout_rx) = mpsc::channel();
    let (stderr_tx, stderr_rx) = mpsc::channel();

    let stream_output = request.stream_output;
    let max = request.output_max_bytes;
    thread::spawn(move || {
        let result = read_and_capture(stdout, max, stream_output, true);
        let _ = stdout_tx.send(result);
    });
    thread::spawn(move || {
        let result = read_and_capture(stderr, max, stream_output, false);
        let _ = stderr_tx.send(result);
    });

    let (status, timed_out) = wait_with_timeout(&mut child, request.timeout_ms)?;

    let stdout_capture = stdout_rx
        .recv()
        .map_err(|_| ExecutorError::Io(std::io::Error::other("stdout channel closed")))?;
    let stderr_capture = stderr_rx
        .recv()
        .map_err(|_| ExecutorError::Io(std::io::Error::other("stderr channel closed")))?;

    let duration_ms = started.elapsed().as_millis();
    Ok(ExecutionResult {
        stdout: stdout_capture.bytes,
        stderr: stderr_capture.bytes,
        exit_code: status.code(),
        duration_ms,
        exit_reason: if timed_out {
            ExitReason::TimeoutKilled
        } else {
            ExitReason::Exited
        },
        stdout_truncated: stdout_capture.truncated,
        stderr_truncated: stderr_capture.truncated,
    })
}

fn build_command(request: &ExecutionRequest) -> Result<Command, ExecutorError> {
    let mut cmd = match &request.command {
        CommandSpec::Argv(argv) => {
            if argv.is_empty() {
                return Err(ExecutorError::EmptyCommand);
            }
            let mut c = Command::new(&argv[0]);
            c.args(&argv[1..]);
            c
        }
        CommandSpec::Shell(shell) => {
            if !request.allow_shell {
                return Err(ExecutorError::ShellDisabled);
            }
            #[cfg(windows)]
            let c = {
                let mut c = Command::new("cmd");
                c.args(["/C", shell]);
                c
            };
            #[cfg(not(windows))]
            let c = {
                let mut c = Command::new("sh");
                c.args(["-c", shell]);
                c
            };
            c
        }
    };

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear();

    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    }
    #[cfg(windows)]
    {
        for key in ["SystemRoot", "WINDIR", "ComSpec", "PATHEXT", "TEMP", "TMP"] {
            if let Ok(value) = std::env::var(key) {
                cmd.env(key, value);
            }
        }
    }

    cmd.env("LC_ALL", "C");
    cmd.env("TZ", "UTC");

    if let Some(seed) = request.seed {
        cmd.env("REPRORUN_SEED", seed.to_string());
    }
    if let Some(time_epoch) = request.time_epoch {
        cmd.env("REPRORUN_TIME_EPOCH", time_epoch.to_string());
    }

    for (k, v) in &request.env {
        cmd.env(k, v);
    }

    if let Some(dir) = &request.working_dir {
        cmd.current_dir(dir);
    }

    Ok(cmd)
}

fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout_ms: Option<u64>,
) -> Result<(std::process::ExitStatus, bool), ExecutorError> {
    match timeout_ms {
        None => child.wait().map(|s| (s, false)).map_err(ExecutorError::Io),
        Some(timeout_ms) => {
            let deadline = Instant::now() + Duration::from_millis(timeout_ms);
            loop {
                if let Some(status) = child.try_wait().map_err(ExecutorError::Io)? {
                    return Ok((status, false));
                }
                if Instant::now() >= deadline {
                    let grace_until = Instant::now() + Duration::from_millis(100);
                    loop {
                        if let Some(status) = child.try_wait().map_err(ExecutorError::Io)? {
                            return Ok((status, false));
                        }
                        if Instant::now() >= grace_until {
                            child.kill().map_err(ExecutorError::Io)?;
                            let status = child.wait().map_err(ExecutorError::Io)?;
                            return Ok((status, true));
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

struct CaptureResult {
    bytes: Vec<u8>,
    truncated: bool,
}

fn read_and_capture<R: Read>(
    mut reader: R,
    max_bytes: usize,
    stream_output: bool,
    stdout_stream: bool,
) -> CaptureResult {
    let mut all = Vec::new();
    let mut truncated = false;
    let mut chunk = [0_u8; 8192];

    loop {
        let Ok(read) = reader.read(&mut chunk) else {
            break;
        };
        if read == 0 {
            break;
        }
        let piece = &chunk[..read];
        if stream_output {
            if stdout_stream {
                let _ = std::io::stdout().write_all(piece);
                let _ = std::io::stdout().flush();
            } else {
                let _ = std::io::stderr().write_all(piece);
                let _ = std::io::stderr().flush();
            }
        }
        let remaining = max_bytes.saturating_sub(all.len());
        if remaining == 0 {
            truncated = true;
            continue;
        }
        let to_copy = remaining.min(piece.len());
        all.extend_from_slice(&piece[..to_copy]);
        if to_copy < piece.len() {
            truncated = true;
        }
    }

    CaptureResult {
        bytes: all,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn platform_command(script: &str) -> CommandSpec {
        #[cfg(windows)]
        {
            CommandSpec::Argv(vec![
                "powershell".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                script.to_string(),
            ])
        }
        #[cfg(not(windows))]
        {
            CommandSpec::Argv(vec!["sh".to_string(), "-c".to_string(), script.to_string()])
        }
    }

    #[test]
    fn rejects_shell_mode_by_default() {
        let req = ExecutionRequest {
            command: CommandSpec::Shell("echo hi".to_string()),
            ..ExecutionRequest::default()
        };
        let err = execute(&req).unwrap_err();
        assert!(matches!(err, ExecutorError::ShellDisabled));
    }

    #[test]
    fn executes_argv_and_captures_stdout() {
        #[cfg(windows)]
        let script = "Write-Output 'hello'";
        #[cfg(not(windows))]
        let script = "printf hello";

        let req = ExecutionRequest {
            command: platform_command(script),
            stream_output: false,
            ..ExecutionRequest::default()
        };
        let out = execute(&req).unwrap();
        assert_eq!(out.exit_reason, ExitReason::Exited);
        assert_eq!(out.exit_code, Some(0));
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("hello"));
    }

    #[test]
    fn injects_stdin() {
        #[cfg(windows)]
        let script = "$x = [Console]::In.ReadToEnd(); Write-Output $x";
        #[cfg(not(windows))]
        let script = "cat";

        let req = ExecutionRequest {
            command: platform_command(script),
            stdin: Some(b"stdin-data".to_vec()),
            stream_output: false,
            ..ExecutionRequest::default()
        };
        let out = execute(&req).unwrap();
        assert_eq!(out.exit_code, Some(0));
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("stdin-data"));
    }

    #[test]
    fn enforces_timeout() {
        #[cfg(windows)]
        let script = "Start-Sleep -Milliseconds 500";
        #[cfg(not(windows))]
        let script = "sleep 1";

        let req = ExecutionRequest {
            command: platform_command(script),
            timeout_ms: Some(50),
            stream_output: false,
            ..ExecutionRequest::default()
        };
        let out = execute(&req).unwrap();
        assert_eq!(out.exit_reason, ExitReason::TimeoutKilled);
    }

    #[test]
    fn truncates_large_output() {
        #[cfg(windows)]
        let script = "$s = 'a' * 10000; Write-Output $s";
        #[cfg(not(windows))]
        let script = "head -c 10000 /dev/zero | tr '\\0' 'a'";

        let req = ExecutionRequest {
            command: platform_command(script),
            output_max_bytes: 100,
            stream_output: false,
            ..ExecutionRequest::default()
        };
        let out = execute(&req).unwrap();
        assert!(out.stdout_truncated);
        assert!(out.stdout.len() <= 100);
    }

    #[test]
    fn preserves_user_provided_sensitive_named_env_vars() {
        #[cfg(windows)]
        let script = "Write-Output $env:API_KEY";
        #[cfg(not(windows))]
        let script = "printf \"$API_KEY\"";

        let mut env = BTreeMap::new();
        env.insert("API_KEY".to_string(), "secret-value".to_string());

        let req = ExecutionRequest {
            command: platform_command(script),
            env,
            stream_output: false,
            ..ExecutionRequest::default()
        };
        let out = execute(&req).unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("secret-value"));
    }
}
