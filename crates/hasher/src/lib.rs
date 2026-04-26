use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use blake3::Hasher;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunHashInput {
    pub command: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub working_dir: PathBuf,
    pub config_bytes: Vec<u8>,
    pub seed: Option<u64>,
    pub time_epoch: Option<i64>,
    pub os: String,
    pub arch: String,
    pub reprorun_version: String,
    pub git_commit: Option<String>,
    pub git_dirty: bool,
}

impl RunHashInput {
    pub fn new(
        command: Vec<String>,
        env: BTreeMap<String, String>,
        working_dir: PathBuf,
        config_bytes: Vec<u8>,
        seed: Option<u64>,
        time_epoch: Option<i64>,
    ) -> Self {
        Self {
            command,
            env,
            working_dir,
            config_bytes,
            seed,
            time_epoch,
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            reprorun_version: env!("CARGO_PKG_VERSION").to_string(),
            git_commit: None,
            git_dirty: false,
        }
    }
}

pub fn hash_run_input(input: &RunHashInput, input_paths: &[PathBuf]) -> Result<String> {
    let mut hasher = Hasher::new();
    hasher.update(b"reprorun-hash-v1");

    update_json(&mut hasher, "command", &input.command)?;
    update_json(&mut hasher, "env", &input.env)?;
    update_json(
        &mut hasher,
        "working_dir",
        &canonical_string(&input.working_dir)?,
    )?;
    update_bytes(&mut hasher, "config_bytes", &input.config_bytes);
    update_json(&mut hasher, "seed", &input.seed)?;
    update_json(&mut hasher, "time_epoch", &input.time_epoch)?;
    update_json(&mut hasher, "os", &input.os)?;
    update_json(&mut hasher, "arch", &input.arch)?;
    update_json(&mut hasher, "reprorun_version", &input.reprorun_version)?;
    update_json(&mut hasher, "git_commit", &input.git_commit)?;
    update_json(&mut hasher, "git_dirty", &input.git_dirty)?;

    let mut normalized_paths = input_paths
        .iter()
        .map(|p| canonical_string(p))
        .collect::<Result<Vec<_>>>()?;
    normalized_paths.sort();

    for path in normalized_paths {
        let bytes = fs::read(&path)
            .with_context(|| format!("failed to read file while hashing input: {path}"))?;
        update_json(&mut hasher, "file_path", &path)?;
        update_bytes(&mut hasher, "file_content", &bytes);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

fn canonical_string(path: &Path) -> Result<String> {
    Ok(path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize path: {}", path.display()))?
        .to_string_lossy()
        .to_string())
}

fn update_json<T: Serialize>(hasher: &mut Hasher, label: &str, value: &T) -> Result<()> {
    hasher.update(label.as_bytes());
    let data = serde_json::to_vec(value)?;
    hasher.update(&data);
    Ok(())
}

fn update_bytes(hasher: &mut Hasher, label: &str, value: &[u8]) {
    hasher.update(label.as_bytes());
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::tempdir;

    fn basic_input(working_dir: PathBuf) -> RunHashInput {
        let mut env = BTreeMap::new();
        env.insert("LC_ALL".to_string(), "C".to_string());
        env.insert("TZ".to_string(), "UTC".to_string());
        RunHashInput {
            command: vec!["echo".to_string(), "hello".to_string()],
            env,
            working_dir,
            config_bytes: b"command: ['echo', 'hello']".to_vec(),
            seed: Some(42),
            time_epoch: Some(1_700_000_000),
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            reprorun_version: "0.1.0".to_string(),
            git_commit: Some("abc123".to_string()),
            git_dirty: false,
        }
    }

    #[test]
    fn hash_is_stable_for_same_input() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("input.txt");
        fs::write(&file_path, "abc").unwrap();
        let input = basic_input(dir.path().to_path_buf());

        let first = hash_run_input(&input, std::slice::from_ref(&file_path)).unwrap();
        let second = hash_run_input(&input, &[file_path]).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn hash_changes_when_file_content_changes() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("input.txt");
        fs::write(&file_path, "abc").unwrap();
        let input = basic_input(dir.path().to_path_buf());

        let first = hash_run_input(&input, std::slice::from_ref(&file_path)).unwrap();
        fs::write(&file_path, "def").unwrap();
        let second = hash_run_input(&input, &[file_path]).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn hash_changes_when_env_changes() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("input.txt");
        fs::write(&file_path, "abc").unwrap();
        let mut input = basic_input(dir.path().to_path_buf());

        let first = hash_run_input(&input, std::slice::from_ref(&file_path)).unwrap();
        input
            .env
            .insert("DEBUG".to_string(), "true".to_string());
        let second = hash_run_input(&input, &[file_path]).unwrap();
        assert_ne!(first, second);
    }

    proptest! {
        #[test]
        fn hash_stable_for_reordered_env_insertion(kv in prop::collection::btree_map("[A-Z]{1,8}", "[a-z0-9]{0,12}", 1..12)) {
            let dir = tempdir().unwrap();
            let file_path = dir.path().join("input.txt");
            fs::write(&file_path, "abc").unwrap();

            let mut a = basic_input(dir.path().to_path_buf());
            let mut b = basic_input(dir.path().to_path_buf());
            a.env.clear();
            b.env.clear();

            let entries: Vec<(String, String)> = kv.into_iter().collect();
            for (k, v) in &entries {
                a.env.insert(k.clone(), v.clone());
            }
            for (k, v) in entries.iter().rev() {
                b.env.insert(k.clone(), v.clone());
            }

            let ha = hash_run_input(&a, std::slice::from_ref(&file_path)).unwrap();
            let hb = hash_run_input(&b, &[file_path]).unwrap();
            prop_assert_eq!(ha, hb);
        }
    }
}
