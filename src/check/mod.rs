pub mod code_trace;
pub mod constraints;
pub mod contracts;
pub mod llm_map;
pub mod plan;
pub mod project_map;
pub mod stack;
pub mod tasks;
pub mod traceability;
pub mod validation;

use colored::Colorize;

use crate::model::milestone::StageStatus;
use crate::model::project::ProjectStatus;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub file: Option<String>,
}

impl Diagnostic {
    pub fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: code.to_string(),
            message: message.into(),
            file: None,
        }
    }
    pub fn warning(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.to_string(),
            message: message.into(),
            file: None,
        }
    }
    pub fn info(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Info,
            code: code.to_string(),
            message: message.into(),
            file: None,
        }
    }
    pub fn with_file(mut self, file: &str) -> Self {
        self.file = Some(file.to_string());
        self
    }
    pub fn print(&self) {
        let (icon, label) = match self.severity {
            Severity::Error => ("✗".red().bold(), "ERR".red().bold()),
            Severity::Warning => ("!".yellow().bold(), "WRN".yellow()),
            Severity::Info => ("·".dimmed(), "INF".dimmed()),
        };
        let loc = match &self.file {
            Some(f) => format!(" {}", f.dimmed()),
            None => String::new(),
        };
        println!(
            "    {} {} [{}] {}{}",
            icon,
            label,
            self.code.dimmed(),
            self.message,
            loc
        );
    }
}

/// Aggregate diagnostics and return exit code: 0=ok/warnings, 1=errors
pub fn exit_code(diags: &[Diagnostic]) -> i32 {
    if diags.iter().any(|d| matches!(d.severity, Severity::Error)) {
        1
    } else {
        0
    }
}

/// Downgrade warnings that are expected at the current project phase to info.
/// Returns the number of diagnostics that were downgraded.
pub fn apply_phase_expectations(diags: &mut [Diagnostic], status: &ProjectStatus) -> usize {
    let label = status.to_string();
    apply_phase_expectations_inner(diags, phase_ord(status), &label)
}

/// Downgrade warnings based on milestone stage status.
pub fn apply_phase_expectations_stage(diags: &mut [Diagnostic], stage: &StageStatus) -> usize {
    let ord = match stage {
        StageStatus::Pending => 2,
        StageStatus::Verified => 3,
        StageStatus::Implementing => 4,
        StageStatus::Implemented => 5,
        StageStatus::Validating => 6,
        StageStatus::Validated => 7,
    };
    let label = stage.to_string();
    apply_phase_expectations_inner(diags, ord, &label)
}

fn apply_phase_expectations_inner(diags: &mut [Diagnostic], current_ord: u8, label: &str) -> usize {
    let mut count = 0;
    for d in diags.iter_mut() {
        if !matches!(d.severity, Severity::Warning) {
            continue;
        }
        // Threshold ordinals:
        // 2 = pending (contracts verified), 3 = implementing,
        // 4 = implemented, 5 = validating
        let threshold = match d.code.as_str() {
            "TRC-020" => 2, // No tests mapped — expected before pending stage
            "TRC-021" => 5, // No gates mapped — expected before validating
            "TRC-030" => 2, // Unmapped requirement — expected before pending stage
            "PLN-040" => 3, // Contract not covered by task — expected before implementing
            "CTR-010" => 4, // Code markers missing — expected before implemented
            "TSK-010" => 4, // Task in_progress too long — expected before implemented
            "TSK-030" => 5, // All tasks done but stage not advanced — expected before validating
            "TSK-050" => 3, // Tracker/plan mismatch — expected before implementing
            _ => continue,
        };
        if current_ord < threshold {
            d.severity = Severity::Info;
            d.message = format!("{} (expected at {} phase)", d.message, label);
            count += 1;
        }
    }
    count
}

fn phase_ord(status: &ProjectStatus) -> u8 {
    match status {
        ProjectStatus::Draft => 0,
        ProjectStatus::Implementing => 3,
        ProjectStatus::Implemented => 4,
        ProjectStatus::Validating => 5,
        ProjectStatus::Validated => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trc021_downgraded_before_validating() {
        let mut diags = vec![Diagnostic::warning(
            "TRC-021",
            "Requirement REQ-001 has no gates mapped",
        )];
        let count = apply_phase_expectations(&mut diags, &ProjectStatus::Draft);
        assert_eq!(count, 1);
        assert!(matches!(diags[0].severity, Severity::Info));
    }

    #[test]
    fn trc021_kept_at_validating() {
        let mut diags = vec![Diagnostic::warning(
            "TRC-021",
            "Requirement REQ-001 has no gates mapped",
        )];
        let count = apply_phase_expectations(&mut diags, &ProjectStatus::Validating);
        assert_eq!(count, 0);
        assert!(matches!(diags[0].severity, Severity::Warning));
    }

    #[test]
    fn trc020_downgraded_at_draft() {
        let mut diags = vec![Diagnostic::warning(
            "TRC-020",
            "Requirement REQ-001 has no tests mapped",
        )];
        let count = apply_phase_expectations(&mut diags, &ProjectStatus::Draft);
        assert_eq!(count, 1);
        assert!(matches!(diags[0].severity, Severity::Info));
    }

    #[test]
    fn trc020_kept_at_implementing() {
        let mut diags = vec![Diagnostic::warning(
            "TRC-020",
            "Requirement REQ-001 has no tests mapped",
        )];
        let count = apply_phase_expectations(&mut diags, &ProjectStatus::Implementing);
        assert_eq!(count, 0);
        assert!(matches!(diags[0].severity, Severity::Warning));
    }

    #[test]
    fn pln040_downgraded_before_implementing() {
        let mut diags = vec![Diagnostic::warning(
            "PLN-040",
            "Contract 'x' not covered by any task",
        )];
        let count = apply_phase_expectations(&mut diags, &ProjectStatus::Draft);
        assert_eq!(count, 1);
        assert!(matches!(diags[0].severity, Severity::Info));
    }

    #[test]
    fn errors_never_downgraded() {
        let mut diags = vec![Diagnostic::error("TRC-021", "should not be touched")];
        let count = apply_phase_expectations(&mut diags, &ProjectStatus::Draft);
        assert_eq!(count, 0);
        assert!(matches!(diags[0].severity, Severity::Error));
    }

    #[test]
    fn stage_pending_downgrades_like_contracts_verified() {
        let mut diags = vec![Diagnostic::warning("CTR-010", "missing @hlv marker")];
        let count = apply_phase_expectations_stage(&mut diags, &StageStatus::Pending);
        assert_eq!(count, 1);
        assert!(matches!(diags[0].severity, Severity::Info));
    }

    #[test]
    fn stage_implemented_keeps_ctr010() {
        let mut diags = vec![Diagnostic::warning("CTR-010", "missing @hlv marker")];
        let count = apply_phase_expectations_stage(&mut diags, &StageStatus::Implemented);
        assert_eq!(count, 0);
        assert!(matches!(diags[0].severity, Severity::Warning));
    }
}
