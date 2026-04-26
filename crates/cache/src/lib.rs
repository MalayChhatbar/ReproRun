use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunMetadata {
    pub hash: String,
    pub exit_code: Option<i32>,
    pub exit_reason: String,
    pub duration_ms: u128,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedRunData {
    pub metadata: RunMetadata,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub config_yaml: String,
    pub env_json: String,
}

fn run_dir(base_dir: &Path, hash: &str) -> PathBuf {
    base_dir.join(".runs").join(hash)
}

pub fn load_run(base_dir: &Path, hash: &str) -> Result<Option<CachedRunData>> {
    validate_hash(hash)?;
    let dir = run_dir(base_dir, hash);
    if !dir.exists() {
        return Ok(None);
    }
    let meta_text = fs::read_to_string(dir.join("meta.json"))
        .with_context(|| format!("failed to read metadata for run '{hash}'"))?;
    let metadata: RunMetadata = serde_json::from_str(&meta_text)
        .with_context(|| format!("failed to parse metadata for run '{hash}'"))?;
    let stdout = fs::read(dir.join("stdout.bin"))
        .with_context(|| format!("failed to read stdout for run '{hash}'"))?;
    let stderr = fs::read(dir.join("stderr.bin"))
        .with_context(|| format!("failed to read stderr for run '{hash}'"))?;
    let config_yaml = fs::read_to_string(dir.join("config.yaml"))
        .with_context(|| format!("failed to read config snapshot for run '{hash}'"))?;
    let env_json = fs::read_to_string(dir.join("env.json"))
        .with_context(|| format!("failed to read env snapshot for run '{hash}'"))?;

    Ok(Some(CachedRunData {
        metadata,
        stdout,
        stderr,
        config_yaml,
        env_json,
    }))
}

pub fn store_run(base_dir: &Path, run: &CachedRunData) -> Result<PathBuf> {
    validate_hash(&run.metadata.hash)?;
    let dir = run_dir(base_dir, &run.metadata.hash);
    fs::create_dir_all(&dir).with_context(|| {
        format!(
            "failed to create run artifact directory '{}'",
            dir.display()
        )
    })?;

    fs::write(
        dir.join("meta.json"),
        serde_json::to_vec_pretty(&run.metadata).context("failed to serialize run metadata")?,
    )?;
    fs::write(dir.join("stdout.bin"), &run.stdout)?;
    fs::write(dir.join("stderr.bin"), &run.stderr)?;
    fs::write(dir.join("config.yaml"), &run.config_yaml)?;
    fs::write(dir.join("env.json"), &run.env_json)?;
    Ok(dir)
}

pub fn has_run(base_dir: &Path, hash: &str) -> bool {
    if validate_hash(hash).is_err() {
        return false;
    }
    run_dir(base_dir, hash).exists()
}

pub fn clean_cache(base_dir: &Path) -> Result<()> {
    let runs_dir = base_dir.join(".runs");
    if runs_dir.exists() {
        fs::remove_dir_all(&runs_dir)
            .with_context(|| format!("failed to remove '{}'", runs_dir.display()))?;
    }
    Ok(())
}

pub fn prune_cache_by_size(base_dir: &Path, max_total_bytes: u64) -> Result<()> {
    let runs_dir = base_dir.join(".runs");
    if !runs_dir.exists() {
        return Ok(());
    }

    let mut entries = fs::read_dir(&runs_dir)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let size = dir_size(&path).ok()?;
            let modified = entry
                .metadata()
                .ok()?
                .modified()
                .ok()
                .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok())
                .map(|dur| dur.as_secs())
                .unwrap_or(0);
            Some((path, size, modified))
        })
        .collect::<Vec<_>>();

    let mut total: u64 = entries.iter().map(|(_, size, _)| *size).sum();
    if total <= max_total_bytes {
        return Ok(());
    }

    entries.sort_by_key(|(_, _, modified)| *modified);
    for (path, size, _) in entries {
        if total <= max_total_bytes {
            break;
        }
        fs::remove_dir_all(&path)
            .with_context(|| format!("failed to prune '{}'", path.display()))?;
        total = total.saturating_sub(size);
    }
    Ok(())
}

fn dir_size(path: &Path) -> Result<u64> {
    let mut total = 0_u64;
    for entry in walkdir::WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            total = total.saturating_add(entry.metadata()?.len());
        }
    }
    Ok(total)
}

fn validate_hash(hash: &str) -> Result<()> {
    if hash.len() != 64 {
        anyhow::bail!("invalid run hash: expected 64 hex chars");
    }
    if !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("invalid run hash: only hex characters are allowed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn hash(ch: char) -> String {
        std::iter::repeat_n(ch, 64).collect()
    }

    fn sample(hash: &str, payload_size: usize) -> CachedRunData {
        CachedRunData {
            metadata: RunMetadata {
                hash: hash.to_string(),
                exit_code: Some(0),
                exit_reason: "exited".to_string(),
                duration_ms: 12,
                stdout_truncated: false,
                stderr_truncated: false,
            },
            stdout: vec![b'a'; payload_size],
            stderr: vec![b'b'; payload_size],
            config_yaml: "command: ['echo', 'ok']".to_string(),
            env_json: "{\"LC_ALL\":\"C\"}".to_string(),
        }
    }

    #[test]
    fn store_and_load_round_trip() {
        let dir = tempdir().unwrap();
        let run = sample(&hash('a'), 16);
        store_run(dir.path(), &run).unwrap();
        let loaded = load_run(dir.path(), &hash('a')).unwrap().unwrap();
        assert_eq!(loaded, run);
    }

    #[test]
    fn has_run_reports_presence() {
        let dir = tempdir().unwrap();
        assert!(!has_run(dir.path(), &hash('a')));
        store_run(dir.path(), &sample(&hash('a'), 8)).unwrap();
        assert!(has_run(dir.path(), &hash('a')));
    }

    #[test]
    fn clean_removes_runs_directory() {
        let dir = tempdir().unwrap();
        store_run(dir.path(), &sample(&hash('a'), 8)).unwrap();
        clean_cache(dir.path()).unwrap();
        assert!(!dir.path().join(".runs").exists());
    }

    #[test]
    fn prune_removes_oldest_entries_first() {
        let dir = tempdir().unwrap();
        store_run(dir.path(), &sample(&hash('a'), 256)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        store_run(dir.path(), &sample(&hash('b'), 256)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        store_run(dir.path(), &sample(&hash('c'), 256)).unwrap();

        prune_cache_by_size(dir.path(), 1_400).unwrap();
        assert!(!has_run(dir.path(), &hash('a')));
        assert!(has_run(dir.path(), &hash('c')));
    }

    #[test]
    fn rejects_invalid_hash_on_load() {
        let dir = tempdir().unwrap();
        let err = load_run(dir.path(), "../escape").unwrap_err();
        assert!(err.to_string().contains("invalid run hash"));
    }

    #[test]
    fn has_run_returns_false_for_invalid_hash() {
        let dir = tempdir().unwrap();
        assert!(!has_run(dir.path(), "../escape"));
    }
}
