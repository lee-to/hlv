use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::check::{Diagnostic, Severity};
use crate::model::policy::ConstraintFile;
use crate::model::project::ProjectMap;

/// Default timeout for check_command execution (60 seconds).
const CHECK_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteMode {
    Single,
    Double,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedCheckCommand {
    program: String,
    args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CheckCommandParseError {
    EmptyCommand,
    MissingProgram,
    UnmatchedQuote,
    UnsupportedSyntax(&'static str),
}

impl CheckCommandParseError {
    fn failure_reason(&self) -> String {
        match self {
            Self::EmptyCommand => "check_command is empty".to_string(),
            Self::MissingProgram => "check_command is missing executable".to_string(),
            Self::UnmatchedQuote => "invalid check_command format (unmatched quote)".to_string(),
            Self::UnsupportedSyntax(op) => format!(
                "unsupported check_command syntax '{}' (use one executable per check_command; shell operators are not supported)",
                op
            ),
        }
    }
}

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
    let parsed = match parse_check_command(cmd) {
        Ok(parsed) => parsed,
        Err(e) => return (false, e.failure_reason()),
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

fn parse_check_command(
    command: &str,
) -> std::result::Result<ParsedCheckCommand, CheckCommandParseError> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(CheckCommandParseError::EmptyCommand);
    }

    if let Some(operator) = find_unsupported_shell_syntax(trimmed) {
        return Err(CheckCommandParseError::UnsupportedSyntax(operator));
    }

    let mut parts = split_command_line(trimmed)?.into_iter();
    let program = parts.next().ok_or(CheckCommandParseError::MissingProgram)?;
    let args = parts.collect();

    Ok(ParsedCheckCommand { program, args })
}

fn split_command_line(command: &str) -> std::result::Result<Vec<String>, CheckCommandParseError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut quote_mode: Option<QuoteMode> = None;

    while let Some(ch) = chars.next() {
        match quote_mode {
            Some(QuoteMode::Single) => {
                if ch == '\'' {
                    quote_mode = None;
                } else {
                    current.push(ch);
                }
            }
            Some(QuoteMode::Double) => {
                if ch == '"' {
                    quote_mode = None;
                } else if ch == '\\' {
                    if let Some(next) = chars.peek().copied() {
                        if next == '"' {
                            current.push(next);
                            let _ = chars.next();
                        } else {
                            current.push(ch);
                        }
                    } else {
                        current.push(ch);
                    }
                } else {
                    current.push(ch);
                }
            }
            None => {
                if ch.is_whitespace() {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                } else if ch == '\'' {
                    quote_mode = Some(QuoteMode::Single);
                } else if ch == '"' {
                    quote_mode = Some(QuoteMode::Double);
                } else {
                    current.push(ch);
                }
            }
        }
    }

    if quote_mode.is_some() {
        return Err(CheckCommandParseError::UnmatchedQuote);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    if tokens.is_empty() {
        return Err(CheckCommandParseError::EmptyCommand);
    }

    Ok(tokens)
}

fn find_unsupported_shell_syntax(command: &str) -> Option<&'static str> {
    let mut chars = command.chars().peekable();
    let mut quote_mode: Option<QuoteMode> = None;

    while let Some(ch) = chars.next() {
        match quote_mode {
            Some(QuoteMode::Single) => {
                if ch == '\'' {
                    quote_mode = None;
                }
            }
            Some(QuoteMode::Double) => {
                if ch == '"' {
                    quote_mode = None;
                } else if ch == '\\' {
                    if let Some(next) = chars.peek().copied() {
                        if next == '"' {
                            let _ = chars.next();
                        }
                    }
                }
            }
            None => match ch {
                '\'' => quote_mode = Some(QuoteMode::Single),
                '"' => quote_mode = Some(QuoteMode::Double),
                '&' => {
                    if chars.peek().copied() == Some('&') {
                        return Some("&&");
                    }
                    return Some("&");
                }
                '|' => {
                    if chars.peek().copied() == Some('|') {
                        return Some("||");
                    }
                    return Some("|");
                }
                ';' => return Some(";"),
                '>' => return Some(">"),
                '<' => return Some("<"),
                '`' => return Some("`"),
                '$' => {
                    if chars.peek().copied() == Some('(') {
                        return Some("$(");
                    }
                }
                _ => {}
            },
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_command_line_supports_quoted_arguments() {
        let tokens = split_command_line(r#"cargo run --message "hello world""#).unwrap();
        assert_eq!(
            tokens,
            vec![
                "cargo".to_string(),
                "run".to_string(),
                "--message".to_string(),
                "hello world".to_string()
            ]
        );
    }

    #[test]
    fn split_command_line_handles_windows_path_in_quotes() {
        let tokens = split_command_line(r#""C:\Program Files\tool.exe" --help"#).unwrap();
        assert_eq!(
            tokens,
            vec![
                r"C:\Program Files\tool.exe".to_string(),
                "--help".to_string()
            ]
        );
    }

    #[test]
    fn split_command_line_rejects_unmatched_quote() {
        let err = split_command_line(r#"cargo --message "broken"#).unwrap_err();
        assert_eq!(err, CheckCommandParseError::UnmatchedQuote);
    }

    #[test]
    fn parse_check_command_rejects_shell_operators() {
        let err = parse_check_command("cargo test && cargo clippy").unwrap_err();
        assert_eq!(err, CheckCommandParseError::UnsupportedSyntax("&&"));
    }

    #[test]
    fn parse_check_command_ignores_operator_chars_inside_quotes() {
        let parsed = parse_check_command(r#"echo "a && b""#).unwrap();
        assert_eq!(parsed.program, "echo");
        assert_eq!(parsed.args, vec!["a && b".to_string()]);
    }
}
