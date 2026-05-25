use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT_OUTPUT_MAX_BYTES: usize = 10 * 1024 * 1024;
const DEFAULT_SNAPSHOT_MAX_BYTES: u64 = 100 * 1024 * 1024;
const DEFAULT_CHECK_RUNS: u32 = 3;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file at {path}: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse YAML config: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("environment variable interpolation failed: {0}")]
    Interpolation(String),
    #[error("invalid config: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReproConfig {
    pub command: CommandSpec,
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
    #[serde(default)]
    pub stdin: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub filesystem: FilesystemConfig,
    #[serde(default)]
    pub limits: LimitsConfig,
    #[serde(default)]
    pub determinism: DeterminismConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub check: CheckConfig,
}

impl ReproConfig {
    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        let raw = fs::read_to_string(path).map_err(|source| ConfigError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_yaml_str(&raw)
    }

    pub fn from_yaml_str(raw: &str) -> Result<Self, ConfigError> {
        let mut cfg: Self = serde_yaml::from_str(raw)?;
        cfg.interpolate_env_vars()?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn interpolate_env_vars(&mut self) -> Result<(), ConfigError> {
        match &mut self.command {
            CommandSpec::Argv(args) => {
                for arg in args {
                    *arg = interpolate_env(arg)?;
                }
            }
            CommandSpec::Shell(command) => {
                *command = interpolate_env(command)?;
            }
        }
        if let Some(value) = &mut self.stdin {
            *value = interpolate_env(value)?;
        }
        for value in self.env.values_mut() {
            *value = interpolate_env(value)?;
        }
        if let Some(path) = &self.working_dir {
            let interpolated = interpolate_env(&path.to_string_lossy())?;
            self.working_dir = Some(PathBuf::from(interpolated));
        }
        for value in &mut self.filesystem.allow {
            *value = PathBuf::from(interpolate_env(&value.to_string_lossy())?);
        }
        for value in &mut self.filesystem.deny {
            *value = PathBuf::from(interpolate_env(&value.to_string_lossy())?);
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), ConfigError> {
        match &self.command {
            CommandSpec::Argv(args) if args.is_empty() => {
                return Err(ConfigError::Validation(
                    "command argv must contain at least one argument".to_string(),
                ));
            }
            CommandSpec::Shell(cmd) if cmd.trim().is_empty() => {
                return Err(ConfigError::Validation(
                    "shell command must not be empty".to_string(),
                ));
            }
            _ => {}
        }

        if self.check.runs == 0 {
            return Err(ConfigError::Validation("check.runs must be >= 1".to_string()));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum CommandSpec {
    Argv(Vec<String>),
    Shell(String),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FilesystemConfig {
    #[serde(default)]
    pub mode: FilesystemMode,
    #[serde(default)]
    pub allow: Vec<PathBuf>,
    #[serde(default)]
    pub deny: Vec<PathBuf>,
    #[serde(default = "default_snapshot_max_bytes")]
    pub snapshot_max_bytes: u64,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            mode: FilesystemMode::Sandbox,
            allow: Vec::new(),
            deny: Vec::new(),
            snapshot_max_bytes: default_snapshot_max_bytes(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FilesystemMode {
    ReadOnly,
    #[default]
    Sandbox,
    Snapshot,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LimitsConfig {
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub cpu_time_secs: Option<u64>,
    #[serde(default)]
    pub memory_mb: Option<u64>,
    #[serde(default)]
    pub process_limit: Option<u32>,
    #[serde(default)]
    pub fd_limit: Option<u32>,
    #[serde(default = "default_output_max_bytes")]
    pub output_max_bytes: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            timeout_secs: None,
            cpu_time_secs: None,
            memory_mb: None,
            process_limit: None,
            fd_limit: None,
            output_max_bytes: default_output_max_bytes(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct DeterminismConfig {
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub time_epoch: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NetworkConfig {
    #[serde(default)]
    pub enabled: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CheckConfig {
    #[serde(default = "default_check_runs")]
    pub runs: u32,
}

impl Default for CheckConfig {
    fn default() -> Self {
        Self {
            runs: default_check_runs(),
        }
    }
}

fn default_output_max_bytes() -> usize {
    DEFAULT_OUTPUT_MAX_BYTES
}

fn default_snapshot_max_bytes() -> u64 {
    DEFAULT_SNAPSHOT_MAX_BYTES
}

fn default_check_runs() -> u32 {
    DEFAULT_CHECK_RUNS
}

fn interpolate_env(input: &str) -> Result<String, ConfigError> {
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(rel_start) = input[cursor..].find("${") {
        let start = cursor + rel_start;
        out.push_str(&input[cursor..start]);
        let var_start = start + 2;
        let Some(rel_end) = input[var_start..].find('}') else {
            return Err(ConfigError::Interpolation(format!(
                "missing closing brace in '{input}'"
            )));
        };
        let end = var_start + rel_end;
        let var_name = &input[var_start..end];
        if var_name.is_empty() {
            return Err(ConfigError::Interpolation(
                "empty variable name in interpolation".to_string(),
            ));
        }
        let value = std::env::var(var_name).map_err(|_| {
            ConfigError::Interpolation(format!("missing environment variable '{var_name}'"))
        })?;
        out.push_str(&value);
        cursor = end + 1;
    }
    out.push_str(&input[cursor..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_config_with_defaults() {
        let cfg = ReproConfig::from_yaml_str("command: [\"echo\", \"hello\"]").unwrap();
        assert_eq!(
            cfg.command,
            CommandSpec::Argv(vec!["echo".to_string(), "hello".to_string()])
        );
        assert_eq!(cfg.filesystem.mode, FilesystemMode::Sandbox);
        assert_eq!(cfg.filesystem.snapshot_max_bytes, DEFAULT_SNAPSHOT_MAX_BYTES);
        assert_eq!(cfg.limits.output_max_bytes, DEFAULT_OUTPUT_MAX_BYTES);
        assert_eq!(cfg.check.runs, DEFAULT_CHECK_RUNS);
        assert!(!cfg.network.enabled);
    }

    #[test]
    fn rejects_unknown_fields() {
        let err = ReproConfig::from_yaml_str(
            r#"
command: ["echo", "hello"]
unknown_field: true
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn interpolates_environment_variables() {
        std::env::set_var("REPRO_TEST_TOKEN", "abc123");
        let cfg = ReproConfig::from_yaml_str(
            r#"
command: ["echo", "${REPRO_TEST_TOKEN}"]
env:
  API_TOKEN: "${REPRO_TEST_TOKEN}"
"#,
        )
        .unwrap();
        assert_eq!(
            cfg.command,
            CommandSpec::Argv(vec!["echo".to_string(), "abc123".to_string()])
        );
        assert_eq!(cfg.env.get("API_TOKEN").unwrap(), "abc123");
    }

    #[test]
    fn interpolation_fails_for_missing_variables() {
        let err = ReproConfig::from_yaml_str(
            r#"
command: ["echo", "${DOES_NOT_EXIST}"]
"#,
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("missing environment variable 'DOES_NOT_EXIST'"));
    }

    #[test]
    fn check_runs_must_be_non_zero() {
        let err = ReproConfig::from_yaml_str(
            r#"
command: ["echo"]
check:
  runs: 0
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("check.runs must be >= 1"));
    }
}
