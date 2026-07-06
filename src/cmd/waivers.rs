use std::path::Path;

use anyhow::Result;
use colored::Colorize;

use super::style;
use crate::check::{Diagnostic, Severity};
use crate::cmd::check::{get_check_report, CheckOptions};
use crate::model::waiver::{Waiver, WaiverFile};

#[derive(Debug, Clone, serde::Serialize)]
pub struct WaiverAudit {
    pub waivers: Vec<Waiver>,
    pub diagnostics: Vec<Diagnostic>,
    pub exit_code: i32,
}

pub fn audit_waivers(root: &Path) -> Result<WaiverAudit> {
    let waiver_path = crate::config_root(root).join("validation/waivers.yaml");
    if !waiver_path.exists() {
        return Ok(WaiverAudit {
            waivers: Vec::new(),
            diagnostics: vec![
                Diagnostic::info("WVR-000", "No validation/waivers.yaml file found")
                    .with_file("validation/waivers.yaml"),
            ],
            exit_code: 0,
        });
    }

    let waiver_file = match WaiverFile::load(&waiver_path) {
        Ok(file) => file,
        Err(e) => {
            let diagnostics = vec![Diagnostic::error(
                "WVR-001",
                format!("Cannot parse validation/waivers.yaml: {e}"),
            )
            .with_file("validation/waivers.yaml")];
            return Ok(WaiverAudit {
                waivers: Vec::new(),
                diagnostics,
                exit_code: 1,
            });
        }
    };

    let report = get_check_report(
        root,
        CheckOptions {
            strict: false,
            with_waivers: false,
        },
    )?;
    let active_keys: std::collections::BTreeSet<(String, String)> = report
        .diagnostics
        .iter()
        .filter_map(|diag| {
            diag.file
                .as_ref()
                .map(|file| (diag.code.to_ascii_uppercase(), file.clone()))
        })
        .collect();

    let mut diagnostics = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let today = chrono::Local::now().date_naive();

    for waiver in &waiver_file.waivers {
        let key = (waiver.code.to_ascii_uppercase(), waiver.file.clone());
        if !seen.insert(key.clone()) {
            diagnostics.push(
                Diagnostic::error(
                    "WVR-010",
                    format!("Duplicate waiver for {} in {}", waiver.code, waiver.file),
                )
                .with_file("validation/waivers.yaml"),
            );
        }
        if waiver.reason.trim().is_empty() {
            diagnostics.push(
                Diagnostic::error(
                    "WVR-011",
                    format!(
                        "Waiver for {} in {} has empty reason",
                        waiver.code, waiver.file
                    ),
                )
                .with_file("validation/waivers.yaml"),
            );
        }
        if waiver.expires < today {
            diagnostics.push(
                Diagnostic::warning(
                    "WVR-020",
                    format!(
                        "Expired waiver for {} in {} expired on {}",
                        waiver.code, waiver.file, waiver.expires
                    ),
                )
                .with_file("validation/waivers.yaml"),
            );
        } else if !active_keys.contains(&key) {
            diagnostics.push(
                Diagnostic::warning(
                    "WVR-030",
                    format!(
                        "Waiver for {} in {} did not match any diagnostic",
                        waiver.code, waiver.file
                    ),
                )
                .with_file("validation/waivers.yaml"),
            );
        }
    }

    let exit_code = if diagnostics
        .iter()
        .any(|d| matches!(d.severity, Severity::Error | Severity::Warning))
    {
        1
    } else {
        0
    };

    Ok(WaiverAudit {
        waivers: waiver_file.waivers,
        diagnostics,
        exit_code,
    })
}

pub fn run_list(root: &Path) -> Result<()> {
    let root = &crate::config_root(root);
    let waiver_path = root.join("validation/waivers.yaml");
    if !waiver_path.exists() {
        style::hint("No validation/waivers.yaml file found");
        return Ok(());
    }
    let waiver_file = WaiverFile::load(&waiver_path)?;
    style::header("waivers");
    if waiver_file.waivers.is_empty() {
        style::hint("No waivers declared");
        return Ok(());
    }
    for waiver in waiver_file.waivers {
        println!(
            "  {} {} {} ({}, expires {})",
            "·".dimmed(),
            waiver.code.bold(),
            waiver.file,
            waiver.reason.dimmed(),
            waiver.expires
        );
    }
    Ok(())
}

pub fn run_audit(root: &Path) -> Result<()> {
    let audit = audit_waivers(root)?;
    style::header("waivers audit");
    if audit.diagnostics.is_empty() {
        style::ok("waivers are valid");
    } else {
        for diag in &audit.diagnostics {
            diag.print();
        }
    }
    if audit.exit_code != 0 {
        std::process::exit(audit.exit_code);
    }
    Ok(())
}
