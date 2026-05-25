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
}

pub fn prepare_sandbox(
    base_dir: &Path,
    config: &ReproConfig,
) -> Result<SandboxLayout, SandboxError> {
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
        for allow in &config.filesystem.allow {
            let resolved = resolve_checked_path(
                base_dir,
                allow,
                &config.filesystem.allow,
                &config.filesystem.deny,
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
    })
}

pub fn resolve_checked_path(
    base_dir: &Path,
    candidate: &Path,
    allowlist: &[PathBuf],
    denylist: &[PathBuf],
) -> Result<PathBuf, SandboxError> {
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
    if resolved_deny.iter().any(|deny| resolved.starts_with(deny)) {
        return Err(SandboxError::DeniedPath {
            path: resolved.display().to_string(),
        });
    }

    let resolved_allow = canonicalize_list(base_dir, allowlist)?;
    if !resolved_allow.is_empty()
        && !resolved_allow
            .iter()
            .any(|allow| resolved.starts_with(allow))
    {
        return Err(SandboxError::OutsideAllowlist {
            path: resolved.display().to_string(),
        });
    }

    Ok(resolved)
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
        let rel = relative_to_base(&canonical_base, source);
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
        let rel = relative_to_base(&canonical_base, path);
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

fn relative_to_base(base: &Path, path: &Path) -> PathBuf {
    if let Ok(rel) = path.strip_prefix(base) {
        return rel.to_path_buf();
    }
    path.file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("unknown"))
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
    }
}
