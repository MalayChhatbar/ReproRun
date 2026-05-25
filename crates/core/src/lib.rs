use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use reprorun_cache::{load_run, store_run, CachedRunData, RunMetadata};
use reprorun_config::{CommandSpec, ReproConfig};
use reprorun_executor::{execute, ExecutionRequest, ExecutionResult, ExitReason};
use reprorun_hasher::{hash_run_input, RunHashInput};
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
    let _layout = prepare_sandbox(base_dir, cfg)?;
    let working_dir = effective_working_dir(base_dir, cfg)?;
    let seed = cfg
        .determinism
        .seed
        .unwrap_or_else(|| deterministic_seed(config_yaml));
    let time_epoch = cfg.determinism.time_epoch.or(Some(0));

    let command_vec = normalize_command_for_hash(&cfg.command);
    let input_files = collect_hash_input_files(base_dir, &cfg.filesystem.allow)?;
    let hash_input = RunHashInput::new(
        command_vec.clone(),
        normalize_env(&cfg.env),
        working_dir.clone(),
        config_yaml.as_bytes().to_vec(),
        Some(seed),
        time_epoch,
    );
    let run_hash = hash_run_input(&hash_input, &input_files)?;

    if options.use_cache {
        if let Some(cached) = load_run(base_dir, &run_hash)? {
            return Ok(RunOutcome {
                hash: run_hash,
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

    let mut exec_env = normalize_env(&cfg.env);
    exec_env.insert("REPRORUN_SEED".to_string(), seed.to_string());
    if let Some(epoch) = time_epoch {
        exec_env.insert("REPRORUN_TIME_EPOCH".to_string(), epoch.to_string());
    }

    let exec_req = ExecutionRequest {
        command: cfg.command.clone(),
        working_dir: Some(working_dir),
        env: exec_env.clone(),
        stdin: cfg.stdin.clone().map(|s| s.into_bytes()),
        timeout_ms: cfg.limits.timeout_secs.map(|s| s.saturating_mul(1000)),
        output_max_bytes: cfg.limits.output_max_bytes,
        stream_output: options.stream_output,
        allow_shell: false,
        seed: Some(seed),
        time_epoch,
    };
    let result = execute(&exec_req)?;
    let result_like = from_execution_result(&result);

    let cached = CachedRunData {
        metadata: RunMetadata {
            hash: run_hash.clone(),
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
        hash: run_hash,
        from_cache: false,
        result: result_like,
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

fn collect_hash_input_files(base_dir: &Path, allow: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let canonical_base = base_dir
        .canonicalize()
        .with_context(|| format!("failed to canonicalize base dir '{}'", base_dir.display()))?;
    let mut files = Vec::new();
    for path in allow {
        let full = if path.is_absolute() {
            path.clone()
        } else {
            base_dir.join(path)
        };
        let canonical_full = full
            .canonicalize()
            .with_context(|| format!("failed to canonicalize allow path '{}'", full.display()))?;
        if !canonical_full.starts_with(&canonical_base) {
            return Err(anyhow!(
                "allow path '{}' escapes base directory '{}'",
                canonical_full.display(),
                canonical_base.display()
            ));
        }
        if canonical_full.is_file() {
            files.push(canonical_full);
            continue;
        }
        if canonical_full.is_dir() {
            for entry in walkdir::WalkDir::new(&canonical_full) {
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
        let outside = std::env::temp_dir().display().to_string().replace('\\', "/");
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
        assert!(err.to_string().contains("outside repository base directory"));
    }
}
