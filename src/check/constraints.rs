use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::check::{Diagnostic, Severity};
use crate::model::policy::ConstraintFile;
use crate::model::project::ProjectMap;
use crate::util::command_parser::{check_command_failure_reason, parse_portable_command};
use crate::util::cwd::ensure_existing_cwd;
use crate::util::text::truncate_ellipsis;

/// Default timeout for check_command execution (60 seconds).
const CHECK_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

/// CST-010: constraint files exist and parse
/// CST-020: no duplicate rule IDs within a constraint
/// CST-030: severity values are valid
pub fn check_constraints(root: &Path, project: &ProjectMap) -> Vec<Diagnostic> {
    let root = &crate::config_root(root);
    let mut diags = Vec::new();
    let valid_severities = ["critical", "high", "medium", "low"];
    let valid_error_levels = ["error", "warning", "info"];

    for entry in &project.constraints {
        let file_path = root.join(&entry.path);

        // CST-010: file exists and parses
        if !file_path.exists() {
            diags.push(
                Diagnostic::error(
                    "CST-010",
                    format!("Constraint file not found: {}", entry.path),
                )
                .with_file(&entry.path),
            );
            continue;
        }

        let cf = match ConstraintFile::load(&file_path) {
            Ok(cf) => cf,
            Err(_) => {
                // Try as performance constraint (metric-based) — not an error
                if crate::model::policy::PerformanceConstraints::load(&file_path).is_ok() {
                    continue;
                }
                diags.push(
                    Diagnostic::error(
                        "CST-010",
                        format!("Cannot parse constraint file: {}", entry.path),
                    )
                    .with_file(&entry.path),
                );
                continue;
            }
        };

        // CST-020: no duplicate rule IDs
        let mut seen_ids = std::collections::HashSet::new();
        for rule in &cf.rules {
            if !seen_ids.insert(&rule.id) {
                diags.push(
                    Diagnostic::error(
                        "CST-020",
                        format!(
                            "Duplicate rule ID '{}' in constraint '{}'",
                            rule.id, entry.id
                        ),
                    )
                    .with_file(&entry.path),
                );
            }

            // CST-030: valid severity
            if !valid_severities.contains(&rule.severity.as_str()) {
                diags.push(
                    Diagnostic::error(
                        "CST-030",
                        format!(
                            "Invalid severity '{}' for rule '{}' in '{}'",
                            rule.severity, rule.id, entry.id
                        ),
                    )
                    .with_file(&entry.path),
                );
            }

            // CST-030: valid error_level (if specified)
            if let Some(ref el) = rule.error_level {
                if !valid_error_levels.contains(&el.as_str()) {
                    diags.push(
                        Diagnostic::error(
                            "CST-030",
                            format!(
                                "Invalid error_level '{}' for rule '{}' in '{}'. Must be: error, warning, info",
                                el, rule.id, entry.id
                            ),
                        )
                        .with_file(&entry.path),
                    );
                }
            }
        }
    }

    diags
}

/// Result of running a single constraint rule check command.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConstraintCheckResult {
    pub constraint_id: String,
    pub rule_id: String,
    pub severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_level: Option<String>,
    pub passed: bool,
    pub message: String,
}

/// Determine diagnostic severity for a failed check command.
/// Priority: error_level (explicit override) > severity mapping.
/// Severity mapping: critical|high → error, medium|low → warning.
fn check_failure_severity(severity: &str, error_level: Option<&str>) -> Severity {
    if let Some(el) = error_level {
        return match el {
            "error" => Severity::Error,
            "info" => Severity::Info,
            _ => Severity::Warning,
        };
    }
    match severity {
        "critical" | "high" => Severity::Error,
        _ => Severity::Warning,
    }
}

/// CST-050: run check_command for constraint rules.
/// Diagnostic level depends on error_level (if set) or severity mapping.
pub fn run_constraint_checks(
    root: &Path,
    project: &ProjectMap,
    filter_constraint: Option<&str>,
    filter_rule: Option<&str>,
) -> (Vec<Diagnostic>, Vec<ConstraintCheckResult>) {
    // Constraint files are config artifacts (config root); check commands
    // execute relative to the repository root.
    let config_root = crate::config_root(root);
    let mut diags = Vec::new();
    let mut results = Vec::new();

    for entry in &project.constraints {
        // Apply constraint filter
        if let Some(filter) = filter_constraint {
            if !entry.id.contains(filter) {
                continue;
            }
        }

        let file_path = config_root.join(&entry.path);
        let cf = match ConstraintFile::load(&file_path) {
            Ok(cf) => cf,
            Err(_) => continue,
        };

        for rule in &cf.rules {
            // Apply rule filter
            if let Some(filter) = filter_rule {
                if rule.id != filter {
                    continue;
                }
            }

            let check_cmd = match &rule.check_command {
                Some(cmd) => cmd,
                None => continue,
            };

            let (passed, message) =
                execute_check_command(check_cmd, root, rule.check_cwd.as_deref());

            if !passed {
                let sev = check_failure_severity(&rule.severity, rule.error_level.as_deref());
                let diag_msg = format!(
                    "Check failed for rule '{}' in '{}': {}",
                    rule.id, entry.id, message
                );
                let diag = Diagnostic {
                    severity: sev,
                    code: "CST-050".to_string(),
                    message: diag_msg,
                    file: Some(entry.path.clone()),
                };
                diags.push(diag);
            }

            results.push(ConstraintCheckResult {
                constraint_id: entry.id.clone(),
                rule_id: rule.id.clone(),
                severity: rule.severity.clone(),
                error_level: rule.error_level.clone(),
                passed,
                message,
            });
        }
    }

    (diags, results)
}

/// CST-060: run file-level check_command for constraint files.
/// Returns diagnostics and results with rule_id = "__file__".
pub fn run_file_level_checks(
    root: &Path,
    project: &ProjectMap,
    filter_constraint: Option<&str>,
) -> (Vec<Diagnostic>, Vec<ConstraintCheckResult>) {
    // Constraint files are config artifacts (config root); check commands
    // execute relative to the repository root.
    let config_root = crate::config_root(root);
    let mut diags = Vec::new();
    let mut results = Vec::new();

    for entry in &project.constraints {
        // Apply constraint filter
        if let Some(filter) = filter_constraint {
            if !entry.id.contains(filter) {
                continue;
            }
        }

        let file_path = config_root.join(&entry.path);
        let cf = match ConstraintFile::load(&file_path) {
            Ok(cf) => cf,
            Err(_) => continue,
        };

        let check_cmd = match &cf.check_command {
            Some(cmd) => cmd,
            None => continue,
        };

        let (passed, message) = execute_check_command(check_cmd, root, cf.check_cwd.as_deref());

        if !passed {
            let diag_msg = format!(
                "File-level check failed for constraint '{}': {}",
                entry.id, message
            );
            diags.push(Diagnostic {
                severity: Severity::Error,
                code: "CST-060".to_string(),
                message: diag_msg,
                file: Some(entry.path.clone()),
            });
        }

        results.push(ConstraintCheckResult {
            constraint_id: entry.id.clone(),
            rule_id: "__file__".to_string(),
            severity: "file".to_string(),
            error_level: None,
            passed,
            message,
        });
    }

    (diags, results)
}

/// Execute a check command and return (passed, message).
fn execute_check_command(cmd: &str, root: &Path, check_cwd: Option<&str>) -> (bool, String) {
    execute_check_command_with_timeout(cmd, root, check_cwd, CHECK_COMMAND_TIMEOUT)
}

fn execute_check_command_with_timeout(
    cmd: &str,
    root: &Path,
    check_cwd: Option<&str>,
    timeout: Duration,
) -> (bool, String) {
    let parsed = match parse_portable_command(cmd) {
        Ok(parsed) => parsed,
        Err(e) => return (false, check_command_failure_reason(&e)),
    };

    let work_dir = match ensure_existing_cwd(root, check_cwd, "check_cwd") {
        Ok((path, _)) => path,
        Err(e) => return (false, e.to_string()),
    };

    let child = Command::new(&parsed.program)
        .args(&parsed.args)
        .current_dir(work_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => return (false, format!("spawn error: {}", e)),
    };

    let mut stdout_handle = child.stdout.take().map(read_pipe_in_thread);
    let mut stderr_handle = child.stderr.take().map(read_pipe_in_thread);

    // Wait with timeout
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = join_pipe_reader(stdout_handle.take());
                let stderr = join_pipe_reader(stderr_handle.take());

                if status.success() {
                    return (true, "ok".to_string());
                } else {
                    let output = if !stderr.trim().is_empty() {
                        stderr.trim().to_string()
                    } else if !stdout.trim().is_empty() {
                        stdout.trim().to_string()
                    } else {
                        format!("exit code {}", status.code().unwrap_or(-1))
                    };
                    let truncated = truncate_check_output(&output);
                    return (false, truncated);
                }
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = join_pipe_reader(stdout_handle.take());
                    let _ = join_pipe_reader(stderr_handle.take());
                    return (false, "timeout".to_string());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = join_pipe_reader(stdout_handle.take());
                let _ = join_pipe_reader(stderr_handle.take());
                return (false, format!("wait error: {}", e));
            }
        }
    }
}

fn read_pipe_in_thread<R>(mut reader: R) -> std::thread::JoinHandle<Vec<u8>>
where
    R: std::io::Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        buf
    })
}

fn join_pipe_reader(handle: Option<std::thread::JoinHandle<Vec<u8>>>) -> String {
    handle
        .and_then(|handle| handle.join().ok())
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default()
}

fn truncate_check_output(output: &str) -> String {
    truncate_ellipsis(output, 200)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command as StdCommand;

    const STDERR_SENTINEL: &str = "stderr sentinel from check_command helper";

    fn quote_command_arg(path: &Path) -> String {
        let value = path.to_string_lossy();
        if value.contains([' ', '\t', '"']) {
            format!("\"{}\"", value.replace('"', "\\\""))
        } else {
            value.into_owned()
        }
    }

    fn build_large_output_helper(dir: &Path) -> PathBuf {
        let src = dir.join("large_output_helper.rs");
        let exe = dir.join(if cfg!(windows) {
            "large_output_helper.exe"
        } else {
            "large_output_helper"
        });

        fs::write(
            &src,
            r#"
use std::io::{self, Write};

fn main() {
    let chunk = vec![b'x'; 8192];

    let mut stdout = io::stdout().lock();
    for _ in 0..256 {
        stdout.write_all(&chunk).unwrap();
    }
    stdout.flush().unwrap();

    let mut stderr = io::stderr().lock();
    stderr
        .write_all(b"stderr sentinel from check_command helper\n")
        .unwrap();
    for _ in 0..256 {
        stderr.write_all(&chunk).unwrap();
    }
    stderr.flush().unwrap();

    std::process::exit(17);
}
"#,
        )
        .unwrap();

        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let output = StdCommand::new(rustc)
            .arg(&src)
            .arg("-o")
            .arg(&exe)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "rustc failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        exe
    }

    #[test]
    fn check_command_output_truncation_does_not_panic_on_multibyte_boundary() {
        let output = format!("{}Жtail", "a".repeat(199));

        let truncated = truncate_check_output(&output);

        assert_eq!(truncated, format!("{}…", "a".repeat(199)));
    }

    #[test]
    fn check_command_drains_large_stdout_and_stderr_without_timeout() {
        let tmp = tempfile::tempdir().unwrap();
        let helper = build_large_output_helper(tmp.path());
        let command = quote_command_arg(&helper);

        let (passed, message) = execute_check_command_with_timeout(
            &command,
            tmp.path(),
            None,
            std::time::Duration::from_secs(5),
        );

        assert!(!passed);
        assert_ne!(message, "timeout");
        assert!(
            message.contains(STDERR_SENTINEL),
            "expected stderr sentinel, got: {message:?}"
        );
    }
}
