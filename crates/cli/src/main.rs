use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Generator, Shell};
use reprorun_cache::{clean_cache, list_runs, load_run, prune_cache_by_size};
use reprorun_core::{
    check_from_yaml, diff_runs_by_hash, explain_hash_from_yaml, load_config_from_file,
    run_from_yaml, RunOptions,
};
use reprorun_reporter::{render_diff_human, render_diff_json};

const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "+",
    env!("REPRORUN_GIT_SHA_SHORT")
);
const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\ncommit: ",
    env!("REPRORUN_GIT_SHA"),
    "\nshort: ",
    env!("REPRORUN_GIT_SHA_SHORT"),
    "\ntag: ",
    env!("REPRORUN_GIT_TAG"),
    "\ndirty: ",
    env!("REPRORUN_GIT_DIRTY")
);

#[derive(Debug, Parser)]
#[command(
    name = "repro",
    version = VERSION,
    long_version = LONG_VERSION,
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
    Inspect(InspectCommand),
    List(ListCommand),
    Doctor(DoctorCommand),
    Hash(HashCommand),
    Completions(CompletionsCommand),
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

#[derive(Debug, Args)]
struct InspectCommand {
    hash: String,
    #[arg(long = "json")]
    json: bool,
}

#[derive(Debug, Args)]
struct ListCommand {
    #[arg(long = "json")]
    json: bool,
    #[arg(long = "limit")]
    limit: Option<usize>,
}

#[derive(Debug, Args)]
struct DoctorCommand {
    #[arg(long = "json")]
    json: bool,
}

#[derive(Debug, Args)]
struct CompletionsCommand {
    shell: Shell,
}

#[derive(Debug, Subcommand)]
enum HashCommands {
    Explain(HashExplainCommand),
}

#[derive(Debug, Args)]
struct HashCommand {
    #[command(subcommand)]
    command: HashCommands,
}

#[derive(Debug, Args)]
struct HashExplainCommand {
    #[arg(default_value = "repro.yaml")]
    config: PathBuf,
    #[arg(long = "json")]
    json: bool,
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

#[derive(Debug, serde::Serialize)]
struct DoctorReport {
    version: String,
    cwd: String,
    config_present: bool,
    cache_entries: usize,
    git_available: bool,
    git_commit: Option<String>,
    git_dirty: bool,
    warnings: Vec<String>,
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
        Commands::Inspect(cmd) => cmd_inspect(&cwd, &cmd),
        Commands::List(cmd) => cmd_list(&cwd, &cmd),
        Commands::Doctor(cmd) => cmd_doctor(&cwd, &cmd),
        Commands::Hash(cmd) => cmd_hash(&cwd, &cmd),
        Commands::Completions(cmd) => cmd_completions(cmd.shell),
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
        if out.from_cache {
            println!("note: cached artifact returned, command was not executed");
        }
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

fn cmd_inspect(base_dir: &Path, cmd: &InspectCommand) -> Result<()> {
    let Some(run) = load_run(base_dir, &cmd.hash)? else {
        return Err(anyhow::anyhow!("run '{}' not found in cache", cmd.hash));
    };

    if cmd.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "hash": run.metadata.hash,
                "exit_code": run.metadata.exit_code,
                "exit_reason": run.metadata.exit_reason,
                "duration_ms": run.metadata.duration_ms,
                "stdout_truncated": run.metadata.stdout_truncated,
                "stderr_truncated": run.metadata.stderr_truncated,
                "stdout_bytes": run.stdout.len(),
                "stderr_bytes": run.stderr.len(),
                "config_yaml": run.config_yaml,
                "env_json": serde_json::from_str::<serde_json::Value>(&run.env_json).unwrap_or(serde_json::Value::String(run.env_json)),
            }))?
        );
    } else {
        println!("hash: {}", run.metadata.hash);
        println!("exit_code: {:?}", run.metadata.exit_code);
        println!("exit_reason: {}", run.metadata.exit_reason);
        println!("duration_ms: {}", run.metadata.duration_ms);
        println!("stdout_bytes: {}", run.stdout.len());
        println!("stderr_bytes: {}", run.stderr.len());
        println!("stdout_truncated: {}", run.metadata.stdout_truncated);
        println!("stderr_truncated: {}", run.metadata.stderr_truncated);
        println!("\n# Config Snapshot\n{}", run.config_yaml);
        println!("# Environment Snapshot\n{}", run.env_json);
    }
    Ok(())
}

fn cmd_list(base_dir: &Path, cmd: &ListCommand) -> Result<()> {
    let mut runs = list_runs(base_dir)?;
    if let Some(limit) = cmd.limit {
        runs.truncate(limit);
    }
    if cmd.json {
        println!("{}", serde_json::to_string_pretty(&runs)?);
    } else if runs.is_empty() {
        println!("no cached runs");
    } else {
        for run in runs {
            println!(
                "{} exit={:?} reason={} duration_ms={} stdout={} stderr={} modified={}",
                run.hash,
                run.exit_code,
                run.exit_reason,
                run.duration_ms,
                run.stdout_bytes,
                run.stderr_bytes,
                run.modified_unix_millis
            );
        }
    }
    Ok(())
}

fn cmd_doctor(base_dir: &Path, cmd: &DoctorCommand) -> Result<()> {
    let report = doctor_report(base_dir)?;
    if cmd.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("version: {}", report.version);
        println!("cwd: {}", report.cwd);
        println!("config_present: {}", report.config_present);
        println!("cache_entries: {}", report.cache_entries);
        println!("git_available: {}", report.git_available);
        println!(
            "git_commit: {}",
            report.git_commit.as_deref().unwrap_or("unknown")
        );
        println!("git_dirty: {}", report.git_dirty);
        if report.warnings.is_empty() {
            println!("warnings: none");
        } else {
            println!("warnings:");
            for warning in report.warnings {
                println!("- {warning}");
            }
        }
    }
    Ok(())
}

fn cmd_hash(base_dir: &Path, cmd: &HashCommand) -> Result<()> {
    match &cmd.command {
        HashCommands::Explain(explain) => cmd_hash_explain(base_dir, explain),
    }
}

fn cmd_hash_explain(base_dir: &Path, cmd: &HashExplainCommand) -> Result<()> {
    let cfg = load_config_from_file(&cmd.config)?;
    let explained = explain_hash_from_yaml(base_dir, &cfg)?;
    if cmd.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "hash": explained.hash,
                "command": explained.command,
                "working_dir": explained.working_dir,
                "seed": explained.seed,
                "time_epoch": explained.time_epoch,
                "git_commit": explained.git_commit,
                "git_dirty": explained.git_dirty,
                "env": explained.env,
                "input_files": explained.input_files
            }))?
        );
    } else {
        println!("hash: {}", explained.hash);
        println!("working_dir: {}", explained.working_dir.display());
        println!("seed: {}", explained.seed);
        println!("time_epoch: {}", explained.time_epoch.unwrap_or(0));
        println!(
            "git_commit: {}",
            explained.git_commit.as_deref().unwrap_or("unknown")
        );
        println!("git_dirty: {}", explained.git_dirty);
        println!("command:");
        for arg in explained.command {
            println!("- {arg}");
        }
        println!("env:");
        for (key, value) in explained.env {
            println!("- {key}={value}");
        }
        println!("input_files:");
        if explained.input_files.is_empty() {
            println!("- none");
        } else {
            for path in explained.input_files {
                println!("- {}", path.display());
            }
        }
    }
    Ok(())
}

fn cmd_completions(shell: Shell) -> Result<()> {
    let mut command = Cli::command();
    print_completions(shell, &mut command);
    Ok(())
}

fn cmd_cache(base_dir: &Path, cmd: &CacheCommand) -> Result<()> {
    match &cmd.command {
        CacheCommands::Clean => clean_cache(base_dir)?,
        CacheCommands::Prune(prune) => prune_cache_by_size(base_dir, prune.max_bytes)?,
    }
    Ok(())
}

fn doctor_report(base_dir: &Path) -> Result<DoctorReport> {
    let cache_entries = list_runs(base_dir)?.len();
    let config_present = base_dir.join("repro.yaml").exists();
    let git_commit = git_stdout(base_dir, &["rev-parse", "HEAD"]);
    let git_available = git_commit.is_some();
    let git_dirty = git_stdout(base_dir, &["status", "--porcelain", "--untracked-files=no"])
        .map(|out| !out.is_empty())
        .unwrap_or(false);

    let mut warnings = Vec::new();
    if !config_present {
        warnings.push("repro.yaml was not found in the current directory".to_string());
    }
    if !git_available {
        warnings.push("git metadata is unavailable; hashes will omit commit context".to_string());
    }
    if git_dirty {
        warnings.push("git working tree has tracked modifications".to_string());
    }

    Ok(DoctorReport {
        version: VERSION.to_string(),
        cwd: base_dir.display().to_string(),
        config_present,
        cache_entries,
        git_available,
        git_commit,
        git_dirty,
        warnings,
    })
}

fn git_stdout(base_dir: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .current_dir(base_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn print_completions<G: Generator>(generator: G, command: &mut clap::Command) {
    generate(
        generator,
        command,
        command.get_name().to_string(),
        &mut std::io::stdout(),
    );
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
                format!("failed to create parent directory for '{}'", path.display())
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

    #[test]
    fn version_metadata_is_embedded() {
        assert!(VERSION.contains(env!("CARGO_PKG_VERSION")));
        assert!(LONG_VERSION.contains("commit:"));
        assert!(LONG_VERSION.contains("tag:"));
        assert!(LONG_VERSION.contains("dirty:"));
    }

    #[test]
    fn default_config_template_is_valid_repro_config() {
        let cfg = reprorun_config::ReproConfig::from_yaml_str(default_config_template()).unwrap();
        assert_eq!(cfg.check.runs, 3);
        assert_eq!(cfg.filesystem.snapshot_max_bytes, 104_857_600);
    }

    #[test]
    fn init_creates_missing_parent_directories() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("configs").join("repro.yaml");
        write_default_config(&path, false).expect("write config");
        assert!(path.exists());
    }

    #[test]
    fn doctor_warns_when_config_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let report = doctor_report(dir.path()).unwrap();
        assert!(!report.config_present);
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("repro.yaml")));
    }

    #[test]
    fn completions_include_binary_name() {
        let mut command = Cli::command();
        let mut buffer = Vec::new();
        generate(Shell::Bash, &mut command, "repro", &mut buffer);
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("repro"));
    }
}
