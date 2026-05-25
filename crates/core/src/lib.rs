use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use reprorun_cache::{load_run, store_run, CachedRunData, RunMetadata};
use reprorun_config::{CommandSpec, ReproConfig};
use reprorun_executor::{execute, ExecutionRequest, ExecutionResult, ExitReason};
use reprorun_hasher::{hash_run_input_from_canonical_paths, RunHashInput};
use reprorun_reporter::{diff_runs, ComparableRun, RunDiff};
use reprorun_sandbox::prepare_sandbox;

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub use_cache: bool,
    pub stream_output: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            use_cache: true,
            stream_output: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub hash: String,
    pub from_cache: bool,
    pub result: ExecutionLikeResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionLikeResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<i32>,
    pub exit_reason: String,
    pub duration_ms: u128,
}

#[derive(Debug, Clone)]
pub struct CheckOutcome {
    pub deterministic: bool,
    pub runs: Vec<RunOutcome>,
    pub first_diff: Option<RunDiff>,
}

#[derive(Debug, Clone)]
pub struct HashExplanation {
    pub hash: String,
    pub command: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub working_dir: PathBuf,
    pub seed: u64,
    pub time_epoch: Option<i64>,
    pub input_files: Vec<PathBuf>,
    pub git_commit: Option<String>,
    pub git_dirty: bool,
}

#[derive(Debug, Clone)]
struct PreparedRun {
    hash: String,
    command: Vec<String>,
    env: BTreeMap<String, String>,
    working_dir: PathBuf,
    seed: u64,
    time_epoch: Option<i64>,
    input_files: Vec<PathBuf>,
    hash_input: RunHashInput,
}

pub fn run_from_yaml(
    base_dir: &Path,
    config_yaml: &str,
    options: RunOptions,
) -> Result<RunOutcome> {
    let cfg = ReproConfig::from_yaml_str(config_yaml)?;
    run_from_config(base_dir, &cfg, config_yaml, options)
}

pub fn run_from_config(
    base_dir: &Path,
    cfg: &ReproConfig,
    config_yaml: &str,
    options: RunOptions,
) -> Result<RunOutcome> {
    let prepared = prepare_run(base_dir, cfg, config_yaml)?;

    if options.use_cache {
        if let Some(cached) = load_run(base_dir, &prepared.hash)? {
            return Ok(RunOutcome {
                hash: prepared.hash,
                from_cache: true,
                result: ExecutionLikeResult {
                    stdout: cached.stdout,
                    stderr: cached.stderr,
                    exit_code: cached.metadata.exit_code,
                    exit_reason: cached.metadata.exit_reason,
                    duration_ms: cached.metadata.duration_ms,
                },
            });
        }
    }

    let mut exec_env = prepared.env.clone();
    exec_env.insert("REPRORUN_SEED".to_string(), prepared.seed.to_string());
    if let Some(epoch) = prepared.time_epoch {
        exec_env.insert("REPRORUN_TIME_EPOCH".to_string(), epoch.to_string());
    }

    let exec_req = ExecutionRequest {
        command: cfg.command.clone(),
        working_dir: Some(prepared.working_dir.clone()),
        env: exec_env.clone(),
        stdin: cfg.stdin.clone().map(|s| s.into_bytes()),
        timeout_ms: cfg.limits.timeout_secs.map(|s| s.saturating_mul(1000)),
        output_max_bytes: cfg.limits.output_max_bytes,
        stream_output: options.stream_output,
        allow_shell: false,
        seed: Some(prepared.seed),
        time_epoch: prepared.time_epoch,
    };
    let result = execute(&exec_req)?;
    let result_like = from_execution_result(&result);

    let cached = CachedRunData {
        metadata: RunMetadata {
            hash: prepared.hash.clone(),
            exit_code: result_like.exit_code,
            exit_reason: result_like.exit_reason.clone(),
            duration_ms: result_like.duration_ms,
            stdout_truncated: result.stdout_truncated,
            stderr_truncated: result.stderr_truncated,
        },
        stdout: result_like.stdout.clone(),
        stderr: result_like.stderr.clone(),
        config_yaml: config_yaml.to_string(),
        env_json: serde_json::to_string_pretty(&exec_env)?,
    };
    store_run(base_dir, &cached)?;

    Ok(RunOutcome {
        hash: prepared.hash,
        from_cache: false,
        result: result_like,
    })
}

pub fn explain_hash_from_yaml(base_dir: &Path, config_yaml: &str) -> Result<HashExplanation> {
    let cfg = ReproConfig::from_yaml_str(config_yaml)?;
    explain_hash_from_config(base_dir, &cfg, config_yaml)
}

pub fn explain_hash_from_config(
    base_dir: &Path,
    cfg: &ReproConfig,
    config_yaml: &str,
) -> Result<HashExplanation> {
    let prepared = prepare_run(base_dir, cfg, config_yaml)?;
    Ok(HashExplanation {
        hash: prepared.hash,
        command: prepared.command,
        env: prepared.env,
        working_dir: prepared.working_dir,
        seed: prepared.seed,
        time_epoch: prepared.time_epoch,
        input_files: prepared.input_files,
        git_commit: prepared.hash_input.git_commit,
        git_dirty: prepared.hash_input.git_dirty,
    })
}

pub fn check_from_yaml(
    base_dir: &Path,
    config_yaml: &str,
    runs_override: Option<u32>,
) -> Result<CheckOutcome> {
    let cfg = ReproConfig::from_yaml_str(config_yaml)?;
    let runs = runs_override.unwrap_or(cfg.check.runs);
    if runs == 0 {
        return Err(anyhow!("check runs must be >= 1"));
    }
    let mut outcomes = Vec::with_capacity(runs as usize);
    for _ in 0..runs {
        outcomes.push(run_from_config(
            base_dir,
            &cfg,
            config_yaml,
            RunOptions {
                use_cache: false,
                stream_output: false,
            },
        )?);
    }

    let baseline = &outcomes[0];
    let mut first_diff = None;
    let mut deterministic = true;
    for run in outcomes.iter().skip(1) {
        let diff = diff_runs(&to_comparable(baseline), &to_comparable(run));
        if diff.different {
            deterministic = false;
            if first_diff.is_none() {
                first_diff = Some(diff);
            }
        }
    }

    Ok(CheckOutcome {
        deterministic,
        runs: outcomes,
        first_diff,
    })
}

pub fn diff_runs_by_hash(base_dir: &Path, left_hash: &str, right_hash: &str) -> Result<RunDiff> {
    let left = load_run(base_dir, left_hash)?
        .ok_or_else(|| anyhow!("run '{left_hash}' not found in cache"))?;
    let right = load_run(base_dir, right_hash)?
        .ok_or_else(|| anyhow!("run '{right_hash}' not found in cache"))?;
    let left_run = ComparableRun {
        id: left_hash.to_string(),
        exit_code: left.metadata.exit_code,
        stdout: left.stdout,
        stderr: left.stderr,
    };
    let right_run = ComparableRun {
        id: right_hash.to_string(),
        exit_code: right.metadata.exit_code,
        stdout: right.stdout,
        stderr: right.stderr,
    };
    Ok(diff_runs(&left_run, &right_run))
}

fn normalize_env(input: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    out.insert("LC_ALL".to_string(), "C".to_string());
    out.insert("TZ".to_string(), "UTC".to_string());
    for (k, v) in input {
        out.insert(k.clone(), v.clone());
    }
    out
}

fn prepare_run(base_dir: &Path, cfg: &ReproConfig, config_yaml: &str) -> Result<PreparedRun> {
    let layout = prepare_sandbox(base_dir, cfg)?;
    let working_dir = effective_working_dir(base_dir, cfg)?;
    let seed = cfg
        .determinism
        .seed
        .unwrap_or_else(|| deterministic_seed(config_yaml));
    let time_epoch = cfg.determinism.time_epoch.or(Some(0));
    let command = normalize_command_for_hash(&cfg.command);
    let env = normalize_env(&cfg.env);
    let input_files = collect_hash_input_files(&layout.resolved_allow_paths)?;

    let mut hash_input = RunHashInput::new(
        command.clone(),
        env.clone(),
        working_dir.clone(),
        config_yaml.as_bytes().to_vec(),
        Some(seed),
        time_epoch,
    );
    let (git_commit, git_dirty) = read_git_metadata(base_dir);
    hash_input.git_commit = git_commit;
    hash_input.git_dirty = git_dirty;
    let hash = hash_run_input_from_canonical_paths(&hash_input, &input_files)?;

    Ok(PreparedRun {
        hash,
        command,
        env,
        working_dir,
        seed,
        time_epoch,
        input_files,
        hash_input,
    })
}

fn effective_working_dir(base_dir: &Path, cfg: &ReproConfig) -> Result<PathBuf> {
    let wd = cfg
        .working_dir
        .clone()
        .unwrap_or_else(|| base_dir.to_path_buf());
    let abs = if wd.is_absolute() {
        wd
    } else {
        base_dir.join(wd)
    };
    abs.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize working directory '{}'",
            abs.display()
        )
    })
}

fn normalize_command_for_hash(command: &CommandSpec) -> Vec<String> {
    match command {
        CommandSpec::Argv(args) => args.clone(),
        CommandSpec::Shell(raw) => vec!["__shell__".to_string(), raw.clone()],
    }
}

fn collect_hash_input_files(allow: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for canonical_path in allow {
        if canonical_path.is_file() {
            files.push(canonical_path.clone());
            continue;
        }
        if canonical_path.is_dir() {
            for entry in walkdir::WalkDir::new(canonical_path) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    files.push(entry.path().to_path_buf());
                }
            }
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn from_execution_result(result: &ExecutionResult) -> ExecutionLikeResult {
    ExecutionLikeResult {
        stdout: result.stdout.clone(),
        stderr: result.stderr.clone(),
        exit_code: result.exit_code,
        exit_reason: match result.exit_reason {
            ExitReason::Exited => "exited".to_string(),
            ExitReason::TimeoutKilled => "timeout_killed".to_string(),
        },
        duration_ms: result.duration_ms,
    }
}

fn to_comparable(outcome: &RunOutcome) -> ComparableRun {
    ComparableRun {
        id: outcome.hash.clone(),
        exit_code: outcome.result.exit_code,
        stdout: outcome.result.stdout.clone(),
        stderr: outcome.result.stderr.clone(),
    }
}

fn deterministic_seed(config_yaml: &str) -> u64 {
    let hash = blake3::hash(config_yaml.as_bytes());
    let bytes = hash.as_bytes();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

fn read_git_metadata(base_dir: &Path) -> (Option<String>, bool) {
    let git_commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(base_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|commit| !commit.is_empty());

    let git_dirty = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .current_dir(base_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty())
        .unwrap_or(false);

    (git_commit, git_dirty)
}

pub fn load_config_from_file(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .with_context(|| format!("failed to read config file '{}'", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_config(dir: &Path, yaml: &str) -> PathBuf {
        let path = dir.join("repro.yaml");
        fs::write(&path, yaml).unwrap();
        path
    }

    fn platform_stable_command_yaml() -> &'static str {
        #[cfg(windows)]
        {
            r#"
command: ["powershell", "-NoProfile", "-Command", "Write-Output 'ok'"]
filesystem:
  mode: sandbox
  allow: []
"#
        }
        #[cfg(not(windows))]
        {
            r#"
command: ["sh", "-c", "printf ok"]
filesystem:
  mode: sandbox
  allow: []
"#
        }
    }

    fn platform_unstable_command_yaml() -> &'static str {
        #[cfg(windows)]
        {
            r#"
command: ["powershell", "-NoProfile", "-Command", "Get-Random"]
filesystem:
  mode: sandbox
  allow: []
"#
        }
        #[cfg(not(windows))]
        {
            r#"
command: ["sh", "-c", "date +%s%N"]
filesystem:
  mode: sandbox
  allow: []
"#
        }
    }

    #[test]
    fn run_caches_result() {
        let dir = tempdir().unwrap();
        let config = platform_stable_command_yaml();
        write_config(dir.path(), config);

        let first = run_from_yaml(
            dir.path(),
            config,
            RunOptions {
                use_cache: true,
                stream_output: false,
            },
        )
        .unwrap();
        assert!(!first.from_cache);

        let second = run_from_yaml(
            dir.path(),
            config,
            RunOptions {
                use_cache: true,
                stream_output: false,
            },
        )
        .unwrap();
        assert!(second.from_cache);
        assert_eq!(first.hash, second.hash);
    }

    #[test]
    fn check_flags_nondeterminism() {
        let dir = tempdir().unwrap();
        let config = platform_unstable_command_yaml();
        write_config(dir.path(), config);

        let check = check_from_yaml(dir.path(), config, Some(3)).unwrap();
        assert_eq!(check.runs.len(), 3);
        assert!(!check.deterministic);
        assert!(check.first_diff.is_some());
    }

    #[test]
    fn rejects_allow_paths_outside_base_dir() {
        let dir = tempdir().unwrap();
        let outside = std::env::temp_dir()
            .display()
            .to_string()
            .replace('\\', "/");
        let config = format!(
            r#"
command: ["echo", "ok"]
filesystem:
  mode: sandbox
  allow:
    - '{}'
"#,
            outside
        );
        let err = run_from_yaml(
            dir.path(),
            &config,
            RunOptions {
                use_cache: false,
                stream_output: false,
            },
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("outside repository base directory"));
    }

    #[test]
    fn check_reports_deterministic_for_stable_command() {
        let dir = tempdir().unwrap();
        let config = platform_stable_command_yaml();
        write_config(dir.path(), config);

        let check = check_from_yaml(dir.path(), config, Some(2)).unwrap();
        assert!(check.deterministic);
        assert!(check.first_diff.is_none());
        assert_eq!(check.runs.len(), 2);
    }

    #[test]
    fn diff_runs_by_hash_reports_output_difference() {
        let dir = tempdir().unwrap();
        let left_hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let right_hash = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

        store_run(
            dir.path(),
            &CachedRunData {
                metadata: RunMetadata {
                    hash: left_hash.to_string(),
                    exit_code: Some(0),
                    exit_reason: "exited".to_string(),
                    duration_ms: 1,
                    stdout_truncated: false,
                    stderr_truncated: false,
                },
                stdout: b"left".to_vec(),
                stderr: Vec::new(),
                config_yaml: "command: ['echo']".to_string(),
                env_json: "{}".to_string(),
            },
        )
        .unwrap();
        store_run(
            dir.path(),
            &CachedRunData {
                metadata: RunMetadata {
                    hash: right_hash.to_string(),
                    exit_code: Some(0),
                    exit_reason: "exited".to_string(),
                    duration_ms: 1,
                    stdout_truncated: false,
                    stderr_truncated: false,
                },
                stdout: b"right".to_vec(),
                stderr: Vec::new(),
                config_yaml: "command: ['echo']".to_string(),
                env_json: "{}".to_string(),
            },
        )
        .unwrap();

        let diff = diff_runs_by_hash(dir.path(), left_hash, right_hash).unwrap();
        assert!(diff.different);
        assert!(diff.stdout.different);
    }

    #[test]
    fn load_config_from_file_round_trips_content() {
        let dir = tempdir().unwrap();
        let path = write_config(dir.path(), "command: ['echo', 'ok']");
        let loaded = load_config_from_file(&path).unwrap();
        assert!(loaded.contains("command:"));
        assert!(loaded.contains("echo"));
    }

    #[test]
    fn explain_hash_reports_seed_and_input_files() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("input.txt");
        fs::write(&input, "abc").unwrap();
        let config = r#"
command: ["echo", "ok"]
filesystem:
  mode: sandbox
  allow:
    - input.txt
"#;

        let explained = explain_hash_from_yaml(dir.path(), config).unwrap();
        assert_eq!(
            explained.command,
            vec!["echo".to_string(), "ok".to_string()]
        );
        assert_eq!(explained.time_epoch, Some(0));
        assert_eq!(explained.input_files.len(), 1);
        assert_eq!(explained.input_files[0], input.canonicalize().unwrap());
        assert!(!explained.hash.is_empty());
    }
}
