use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::check::{Diagnostic, Severity};
use crate::model::policy::ConstraintFile;
use crate::model::project::ProjectMap;

/// Default timeout for check_command execution (60 seconds).
const CHECK_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

/// CST-010: constraint files exist and parse
/// CST-020: no duplicate rule IDs within a constraint
/// CST-030: severity values are valid
pub fn check_constraints(root: &Path, project: &ProjectMap) -> Vec<Diagnostic> {
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
    let mut diags = Vec::new();
    let mut results = Vec::new();

    for entry in &project.constraints {
        // Apply constraint filter
        if let Some(filter) = filter_constraint {
            if !entry.id.contains(filter) {
                continue;
            }
        }

        let file_path = root.join(&entry.path);
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

            let work_dir = match &rule.check_cwd {
                Some(rel) => root.join(rel),
                None => root.to_path_buf(),
            };

            let (passed, message) = execute_check_command(check_cmd, &work_dir);

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
    let mut diags = Vec::new();
    let mut results = Vec::new();

    for entry in &project.constraints {
        // Apply constraint filter
        if let Some(filter) = filter_constraint {
            if !entry.id.contains(filter) {
                continue;
            }
        }

        let file_path = root.join(&entry.path);
        let cf = match ConstraintFile::load(&file_path) {
            Ok(cf) => cf,
            Err(_) => continue,
        };

        let check_cmd = match &cf.check_command {
            Some(cmd) => cmd,
            None => continue,
        };

        let work_dir = match &cf.check_cwd {
            Some(rel) => root.join(rel),
            None => root.to_path_buf(),
        };

        let (passed, message) = execute_check_command(check_cmd, &work_dir);

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
fn execute_check_command(cmd: &str, work_dir: &Path) -> (bool, String) {
    let child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(work_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => return (false, format!("spawn error: {}", e)),
    };

    // Wait with timeout
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        let mut buf = String::new();
                        std::io::Read::read_to_string(&mut s, &mut buf).ok();
                        buf
                    })
                    .unwrap_or_default();
                let stderr = child
                    .stderr
                    .take()
                    .map(|mut s| {
                        let mut buf = String::new();
                        std::io::Read::read_to_string(&mut s, &mut buf).ok();
                        buf
                    })
                    .unwrap_or_default();

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
                    // Truncate long output
                    let truncated = if output.len() > 200 {
                        format!("{}...", &output[..200])
                    } else {
                        output
                    };
                    return (false, truncated);
                }
            }
            Ok(None) => {
                if start.elapsed() > CHECK_COMMAND_TIMEOUT {
                    let _ = child.kill();
                    return (false, "timeout".to_string());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return (false, format!("wait error: {}", e)),
        }
    }
}
