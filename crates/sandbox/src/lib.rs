use std::fs;
use std::path::{Path, PathBuf};

use reprorun_config::{FilesystemMode, ReproConfig};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("sandbox path resolution failed for '{path}': {source}")]
    Canonicalize {
        path: String,
        source: std::io::Error,
    },
    #[error("path '{path}' is denied by policy")]
    DeniedPath { path: String },
    #[error("path '{path}' is outside allowlist")]
    OutsideAllowlist { path: String },
    #[error("path '{path}' is outside repository base directory")]
    OutsideBase { path: String },
    #[error("snapshot exceeds size limit: {actual} > {limit} bytes")]
    SnapshotTooLarge { actual: u64, limit: u64 },
    #[error("sandbox I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct SandboxLayout {
    pub root: PathBuf,
    pub snapshot_root: PathBuf,
    pub total_snapshot_bytes: u64,
    pub resolved_allow_paths: Vec<PathBuf>,
}

pub fn prepare_sandbox(
    base_dir: &Path,
    config: &ReproConfig,
) -> Result<SandboxLayout, SandboxError> {
    let canonical_base = canonicalize_base_dir(base_dir)?;
    let resolved_allow = canonicalize_list(base_dir, &config.filesystem.allow)?;
    let resolved_deny = canonicalize_list(base_dir, &config.filesystem.deny)?;

    let tmp_root = base_dir.join("tmp").join("sandbox");
    fs::create_dir_all(&tmp_root)?;
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos().to_string())
        .unwrap_or_else(|_| "0".to_string());
    let snapshot_root = tmp_root.join(format!("snapshot-{run_id}"));
    fs::create_dir_all(&snapshot_root)?;

    let mut total = 0_u64;
    if matches!(
        config.filesystem.mode,
        FilesystemMode::Sandbox | FilesystemMode::Snapshot
    ) {
        for resolved in &resolved_allow {
            let resolved = resolve_checked_path_with_resolved_lists(
                &canonical_base,
                resolved,
                &resolved_allow,
                &resolved_deny,
            )?;
            total = total.saturating_add(copy_into_snapshot(base_dir, &resolved, &snapshot_root)?);
            if total > config.filesystem.snapshot_max_bytes {
                return Err(SandboxError::SnapshotTooLarge {
                    actual: total,
                    limit: config.filesystem.snapshot_max_bytes,
                });
            }
        }
    }

    Ok(SandboxLayout {
        root: tmp_root,
        snapshot_root,
        total_snapshot_bytes: total,
        resolved_allow_paths: resolved_allow,
    })
}

pub fn resolve_checked_path(
    base_dir: &Path,
    candidate: &Path,
    allowlist: &[PathBuf],
    denylist: &[PathBuf],
) -> Result<PathBuf, SandboxError> {
    let canonical_base = canonicalize_base_dir(base_dir)?;
    let absolute_candidate = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base_dir.join(candidate)
    };
    let resolved =
        absolute_candidate
            .canonicalize()
            .map_err(|source| SandboxError::Canonicalize {
                path: candidate.display().to_string(),
                source,
            })?;
    let resolved_deny = canonicalize_list(base_dir, denylist)?;
    let resolved_allow = canonicalize_list(base_dir, allowlist)?;
    resolve_checked_path_with_resolved_lists(
        &canonical_base,
        &resolved,
        &resolved_allow,
        &resolved_deny,
    )
}

fn resolve_checked_path_with_resolved_lists(
    canonical_base: &Path,
    resolved: &Path,
    resolved_allow: &[PathBuf],
    resolved_deny: &[PathBuf],
) -> Result<PathBuf, SandboxError> {
    if !resolved.starts_with(canonical_base) {
        return Err(SandboxError::OutsideBase {
            path: resolved.display().to_string(),
        });
    }
    if resolved_deny.iter().any(|deny| resolved.starts_with(deny)) {
        return Err(SandboxError::DeniedPath {
            path: resolved.display().to_string(),
        });
    }
    if !resolved_allow.is_empty()
        && !resolved_allow
            .iter()
            .any(|allow| resolved.starts_with(allow))
    {
        return Err(SandboxError::OutsideAllowlist {
            path: resolved.display().to_string(),
        });
    }

    Ok(resolved.to_path_buf())
}

fn canonicalize_list(base_dir: &Path, paths: &[PathBuf]) -> Result<Vec<PathBuf>, SandboxError> {
    paths
        .iter()
        .map(|path| {
            let absolute = if path.is_absolute() {
                path.to_path_buf()
            } else {
                base_dir.join(path)
            };
            absolute
                .canonicalize()
                .map_err(|source| SandboxError::Canonicalize {
                    path: path.display().to_string(),
                    source,
                })
        })
        .collect()
}

fn canonicalize_base_dir(base_dir: &Path) -> Result<PathBuf, SandboxError> {
    base_dir
        .canonicalize()
        .map_err(|source| SandboxError::Canonicalize {
            path: base_dir.display().to_string(),
            source,
        })
}

fn copy_into_snapshot(
    base_dir: &Path,
    source: &Path,
    snapshot_root: &Path,
) -> Result<u64, SandboxError> {
    let canonical_base = base_dir
        .canonicalize()
        .map_err(|source| SandboxError::Canonicalize {
            path: base_dir.display().to_string(),
            source,
        })?;
    let mut copied = 0_u64;
    if source.is_file() {
        let rel = relative_to_base(&canonical_base, source)?;
        let dest = snapshot_root.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, &dest)?;
        copied = copied.saturating_add(source.metadata()?.len());
        return Ok(copied);
    }

    for entry in walkdir::WalkDir::new(source) {
        let entry = entry.map_err(|e| std::io::Error::other(e.to_string()))?;
        let path = entry.path();
        let rel = relative_to_base(&canonical_base, path)?;
        let dest = snapshot_root.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest)?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(path, &dest)?;
        copied = copied.saturating_add(path.metadata()?.len());
    }
    Ok(copied)
}

fn relative_to_base(base: &Path, path: &Path) -> Result<PathBuf, SandboxError> {
    let rel = path
        .strip_prefix(base)
        .map_err(|_| SandboxError::OutsideBase {
            path: path.display().to_string(),
        })?;
    Ok(rel.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reprorun_config::ReproConfig;
    use tempfile::tempdir;

    fn config_with_allow(path: &str) -> ReproConfig {
        ReproConfig::from_yaml_str(&format!(
            r#"
command: ["echo", "ok"]
filesystem:
  mode: sandbox
  allow:
    - "{path}"
  snapshot_max_bytes: 104857600
"#
        ))
        .unwrap()
    }

    #[test]
    fn resolves_allowed_path() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.txt"), "ok").unwrap();

        let cfg = config_with_allow("src");
        let resolved = resolve_checked_path(
            dir.path(),
            Path::new("src"),
            &cfg.filesystem.allow,
            &cfg.filesystem.deny,
        )
        .unwrap();
        assert!(resolved.ends_with("src"));
    }

    #[test]
    fn rejects_denied_path() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.txt"), "ok").unwrap();

        let cfg = ReproConfig::from_yaml_str(
            r#"
command: ["echo", "ok"]
filesystem:
  mode: sandbox
  allow:
    - "src"
  deny:
    - "src"
"#,
        )
        .unwrap();
        let err = resolve_checked_path(
            dir.path(),
            Path::new("src"),
            &cfg.filesystem.allow,
            &cfg.filesystem.deny,
        )
        .unwrap_err();
        assert!(matches!(err, SandboxError::DeniedPath { .. }));
    }

    #[test]
    fn snapshots_allowed_files() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.txt"), "ok").unwrap();

        let cfg = config_with_allow("src");
        let layout = prepare_sandbox(dir.path(), &cfg).unwrap();
        assert!(layout.snapshot_root.join("src").join("a.txt").exists());
        assert!(layout.total_snapshot_bytes > 0);
        assert_eq!(layout.resolved_allow_paths.len(), 1);
    }

    #[test]
    fn rejects_paths_outside_base_dir() {
        let dir = tempdir().unwrap();
        let outside = std::env::temp_dir().display().to_string().replace('\\', "/");
        let cfg = ReproConfig::from_yaml_str(&format!(
            r#"
command: ["echo", "ok"]
filesystem:
  mode: sandbox
  allow:
    - '{}'
"#,
            outside
        ))
        .unwrap();
        let err = prepare_sandbox(dir.path(), &cfg).unwrap_err();
        assert!(matches!(err, SandboxError::OutsideBase { .. }));
    }

    #[test]
    fn read_only_mode_skips_snapshot_copy() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.txt"), "ok").unwrap();

        let cfg = ReproConfig::from_yaml_str(
            r#"
command: ["echo", "ok"]
filesystem:
  mode: read_only
  allow:
    - "src"
"#,
        )
        .unwrap();
        let layout = prepare_sandbox(dir.path(), &cfg).unwrap();
        assert_eq!(layout.total_snapshot_bytes, 0);
        assert!(!layout.snapshot_root.join("src").join("a.txt").exists());
        assert_eq!(layout.resolved_allow_paths.len(), 1);
    }

    #[test]
    fn snapshot_limit_is_enforced() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.txt"), "0123456789").unwrap();

        let cfg = ReproConfig::from_yaml_str(
            r#"
command: ["echo", "ok"]
filesystem:
  mode: sandbox
  allow:
    - "src"
  snapshot_max_bytes: 1
"#,
        )
        .unwrap();
        let err = prepare_sandbox(dir.path(), &cfg).unwrap_err();
        assert!(matches!(err, SandboxError::SnapshotTooLarge { .. }));
    }
}
