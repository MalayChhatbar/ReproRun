use colored::Colorize;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComparableRun {
    pub id: String,
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldDiff {
    pub different: bool,
    pub unified: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunDiff {
    pub different: bool,
    pub exit_code: FieldDiff,
    pub stdout: FieldDiff,
    pub stderr: FieldDiff,
}

pub fn diff_runs(left: &ComparableRun, right: &ComparableRun) -> RunDiff {
    let exit_diff = left.exit_code != right.exit_code;
    let stdout_diff = left.stdout != right.stdout;
    let stderr_diff = left.stderr != right.stderr;

    RunDiff {
        different: exit_diff || stdout_diff || stderr_diff,
        exit_code: FieldDiff {
            different: exit_diff,
            unified: if exit_diff {
                Some(format!(
                    "--- {}\n+++ {}\n-{:?}\n+{:?}",
                    left.id, right.id, left.exit_code, right.exit_code
                ))
            } else {
                None
            },
        },
        stdout: FieldDiff {
            different: stdout_diff,
            unified: if stdout_diff {
                Some(unified_text_diff(
                    &left.id,
                    &right.id,
                    &String::from_utf8_lossy(&left.stdout),
                    &String::from_utf8_lossy(&right.stdout),
                ))
            } else {
                None
            },
        },
        stderr: FieldDiff {
            different: stderr_diff,
            unified: if stderr_diff {
                Some(unified_text_diff(
                    &left.id,
                    &right.id,
                    &String::from_utf8_lossy(&left.stderr),
                    &String::from_utf8_lossy(&right.stderr),
                ))
            } else {
                None
            },
        },
    }
}

pub fn render_diff_human(diff: &RunDiff, color: bool) -> String {
    if !diff.different {
        return "No differences detected.".to_string();
    }

    let mut out = String::new();
    if diff.exit_code.different {
        out.push_str("## Exit Code\n");
        out.push_str(&decorate_diff(
            diff.exit_code.unified.as_deref().unwrap_or_default(),
            color,
        ));
        out.push('\n');
    }
    if diff.stdout.different {
        out.push_str("## Stdout\n");
        out.push_str(&decorate_diff(
            diff.stdout.unified.as_deref().unwrap_or_default(),
            color,
        ));
        out.push('\n');
    }
    if diff.stderr.different {
        out.push_str("## Stderr\n");
        out.push_str(&decorate_diff(
            diff.stderr.unified.as_deref().unwrap_or_default(),
            color,
        ));
        out.push('\n');
    }
    out
}

pub fn render_diff_json(diff: &RunDiff) -> serde_json::Result<String> {
    serde_json::to_string_pretty(diff)
}

fn unified_text_diff(left_id: &str, right_id: &str, left: &str, right: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("--- {left_id}\n+++ {right_id}\n"));

    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();
    let common = left_lines.len().max(right_lines.len());
    for i in 0..common {
        match (left_lines.get(i), right_lines.get(i)) {
            (Some(a), Some(b)) if a == b => out.push_str(&format!(" {a}\n")),
            (Some(a), Some(b)) => {
                out.push_str(&format!("-{a}\n"));
                out.push_str(&format!("+{b}\n"));
            }
            (Some(a), None) => out.push_str(&format!("-{a}\n")),
            (None, Some(b)) => out.push_str(&format!("+{b}\n")),
            (None, None) => {}
        }
    }
    out
}

fn decorate_diff(text: &str, color: bool) -> String {
    if !color {
        return text.to_string();
    }
    let mut out = String::new();
    for line in text.lines() {
        let rendered = if line.starts_with("+++") || line.starts_with("---") {
            line.bold().to_string()
        } else if line.starts_with('+') {
            line.green().to_string()
        } else if line.starts_with('-') {
            line.red().to_string()
        } else {
            line.to_string()
        };
        out.push_str(&rendered);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_no_diff_when_equal() {
        let a = ComparableRun {
            id: "run-a".to_string(),
            exit_code: Some(0),
            stdout: b"same".to_vec(),
            stderr: Vec::new(),
        };
        let b = a.clone();
        let diff = diff_runs(&a, &b);
        assert!(!diff.different);
        assert_eq!(render_diff_human(&diff, false), "No differences detected.");
    }

    #[test]
    fn reports_stdout_diff() {
        let a = ComparableRun {
            id: "run-a".to_string(),
            exit_code: Some(0),
            stdout: b"hello\nworld\n".to_vec(),
            stderr: Vec::new(),
        };
        let b = ComparableRun {
            id: "run-b".to_string(),
            exit_code: Some(0),
            stdout: b"hello\nearth\n".to_vec(),
            stderr: Vec::new(),
        };
        let diff = diff_runs(&a, &b);
        assert!(diff.different);
        assert!(diff.stdout.different);
        let human = render_diff_human(&diff, false);
        assert!(human.contains("-world"));
        assert!(human.contains("+earth"));
    }

    #[test]
    fn renders_json() {
        let a = ComparableRun {
            id: "run-a".to_string(),
            exit_code: Some(0),
            stdout: b"a".to_vec(),
            stderr: Vec::new(),
        };
        let b = ComparableRun {
            id: "run-b".to_string(),
            exit_code: Some(1),
            stdout: b"b".to_vec(),
            stderr: Vec::new(),
        };
        let diff = diff_runs(&a, &b);
        let json = render_diff_json(&diff).unwrap();
        assert!(json.contains("\"different\": true"));
    }
}
