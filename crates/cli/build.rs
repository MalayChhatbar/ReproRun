use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../.git/HEAD");

    let git_sha = run_git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    let git_sha_short = run_git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    let git_tag = run_git(&["describe", "--tags", "--exact-match"]).unwrap_or_else(|| "none".to_string());
    let git_dirty = is_dirty().to_string();

    println!("cargo:rustc-env=REPRORUN_GIT_SHA={git_sha}");
    println!("cargo:rustc-env=REPRORUN_GIT_SHA_SHORT={git_sha_short}");
    println!("cargo:rustc-env=REPRORUN_GIT_TAG={git_tag}");
    println!("cargo:rustc-env=REPRORUN_GIT_DIRTY={git_dirty}");
}

fn run_git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed)
}

fn is_dirty() -> bool {
    match Command::new("git")
        .args(["status", "--porcelain"])
        .output()
    {
        Ok(output) if output.status.success() => !output.stdout.is_empty(),
        _ => false,
    }
}
