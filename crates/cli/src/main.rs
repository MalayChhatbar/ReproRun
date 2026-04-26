use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use reprorun_cache::{clean_cache, prune_cache_by_size};
use reprorun_core::{
    check_from_yaml, diff_runs_by_hash, load_config_from_file, run_from_yaml, RunOptions,
};
use reprorun_reporter::{render_diff_human, render_diff_json};

#[derive(Debug, Parser)]
#[command(
    name = "repro",
    about = "Deterministic command execution with reproducible outputs"
)]
struct Cli {
    #[arg(short = 'q', long = "quiet", global = true)]
    quiet: bool,
    #[arg(short = 'v', action = clap::ArgAction::Count, global = true)]
    verbose: u8,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init(InitCommand),
    Run(RunCommand),
    Check(CheckCommand),
    Diff(DiffCommand),
    Cache(CacheCommand),
}

#[derive(Debug, Args)]
struct InitCommand {
    #[arg(default_value = "repro.yaml")]
    path: PathBuf,
    #[arg(long = "force")]
    force: bool,
}

#[derive(Debug, Args)]
struct RunCommand {
    #[arg(default_value = "repro.yaml")]
    config: PathBuf,
    #[arg(long = "no-cache")]
    no_cache: bool,
    #[arg(long = "json")]
    json: bool,
}

#[derive(Debug, Args)]
struct CheckCommand {
    #[arg(default_value = "repro.yaml")]
    config: PathBuf,
    #[arg(long = "runs")]
    runs: Option<u32>,
    #[arg(long = "json")]
    json: bool,
}

#[derive(Debug, Args)]
struct DiffCommand {
    left: String,
    right: String,
    #[arg(long = "json")]
    json: bool,
    #[arg(long = "no-color")]
    no_color: bool,
}

#[derive(Debug, Subcommand)]
enum CacheCommands {
    Clean,
    Prune(PruneCommand),
}

#[derive(Debug, Args)]
struct CacheCommand {
    #[command(subcommand)]
    command: CacheCommands,
}

#[derive(Debug, Args)]
struct PruneCommand {
    #[arg(long = "max-bytes")]
    max_bytes: u64,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(2);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir().context("failed to get current directory")?;
    let stream_output = !cli.quiet;
    let _verbosity = cli.verbose;

    match cli.command {
        Commands::Init(cmd) => cmd_init(&cmd),
        Commands::Run(cmd) => cmd_run(&cwd, &cmd, stream_output),
        Commands::Check(cmd) => cmd_check(&cwd, &cmd),
        Commands::Diff(cmd) => cmd_diff(&cwd, &cmd),
        Commands::Cache(cmd) => cmd_cache(&cwd, &cmd),
    }
}

fn cmd_init(cmd: &InitCommand) -> Result<()> {
    write_default_config(&cmd.path, cmd.force)?;
    println!("created {}", cmd.path.display());
    Ok(())
}

fn cmd_run(base_dir: &Path, cmd: &RunCommand, stream_output: bool) -> Result<()> {
    let cfg = load_config_from_file(&cmd.config)?;
    let out = run_from_yaml(
        base_dir,
        &cfg,
        RunOptions {
            use_cache: !cmd.no_cache,
            stream_output,
        },
    )?;
    if cmd.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "hash": out.hash,
                "from_cache": out.from_cache,
                "exit_code": out.result.exit_code,
                "exit_reason": out.result.exit_reason,
                "duration_ms": out.result.duration_ms
            }))?
        );
    } else {
        println!("hash: {}", out.hash);
        println!("from_cache: {}", out.from_cache);
        println!("exit_code: {:?}", out.result.exit_code);
        println!("exit_reason: {}", out.result.exit_reason);
    }
    Ok(())
}

fn cmd_check(base_dir: &Path, cmd: &CheckCommand) -> Result<()> {
    let cfg = load_config_from_file(&cmd.config)?;
    let check = check_from_yaml(base_dir, &cfg, cmd.runs)?;
    if cmd.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "deterministic": check.deterministic,
                "run_hashes": check.runs.iter().map(|r| r.hash.clone()).collect::<Vec<_>>(),
                "diff": check.first_diff
            }))?
        );
    } else {
        println!("deterministic: {}", check.deterministic);
        for (idx, run) in check.runs.iter().enumerate() {
            println!(
                "run[{idx}] hash={} exit={:?}",
                run.hash, run.result.exit_code
            );
        }
        if let Some(diff) = &check.first_diff {
            println!("{}", render_diff_human(diff, true));
        }
    }
    if !check.deterministic {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_diff(base_dir: &Path, cmd: &DiffCommand) -> Result<()> {
    let diff = diff_runs_by_hash(base_dir, &cmd.left, &cmd.right)?;
    if cmd.json {
        println!("{}", render_diff_json(&diff)?);
    } else {
        println!("{}", render_diff_human(&diff, !cmd.no_color));
    }
    if diff.different {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_cache(base_dir: &Path, cmd: &CacheCommand) -> Result<()> {
    match &cmd.command {
        CacheCommands::Clean => clean_cache(base_dir)?,
        CacheCommands::Prune(prune) => prune_cache_by_size(base_dir, prune.max_bytes)?,
    }
    Ok(())
}

fn write_default_config(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        return Err(anyhow::anyhow!(
            "config file '{}' already exists (use --force to overwrite)",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create parent directory for '{}'",
                    path.display()
                )
            })?;
        }
    }
    std::fs::write(path, default_config_template())
        .with_context(|| format!("failed to write '{}'", path.display()))?;
    Ok(())
}

fn default_config_template() -> &'static str {
    r#"command: ["echo", "hello from ReproRun"]

env:
  DEBUG: "false"

filesystem:
  mode: sandbox
  allow: []
  snapshot_max_bytes: 104857600

limits:
  timeout_secs: 5
  output_max_bytes: 10485760

determinism:
  seed: 42
  time_epoch: 1700000000

check:
  runs: 3
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_writes_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("repro.yaml");
        write_default_config(&path, false).expect("write config");
        let content = std::fs::read_to_string(path).expect("read config");
        assert!(content.contains("command:"));
        assert!(content.contains("filesystem:"));
    }

    #[test]
    fn init_refuses_overwrite_without_force() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("repro.yaml");
        std::fs::write(&path, "existing: true").expect("seed");
        let err = write_default_config(&path, false).expect_err("must fail");
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn init_overwrites_with_force() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("repro.yaml");
        std::fs::write(&path, "existing: true").expect("seed");
        write_default_config(&path, true).expect("force write");
        let content = std::fs::read_to_string(path).expect("read config");
        assert!(content.contains("hello from ReproRun"));
        assert!(!content.contains("existing: true"));
    }
}
