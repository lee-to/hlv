use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::style;
use crate::model::milestone::{
    ContractChange, HistoryEntry, MilestoneCurrent, MilestoneMap, MilestoneStatus, StageStatus,
};
use crate::model::project::ProjectMap;

pub fn run_new(root: &Path, name: &str) -> Result<()> {
    style::header("milestone new");

    let mut map = load_or_create(root)?;

    if let Some(current) = &map.current {
        anyhow::bail!(
            "Active milestone exists: {}. Run `hlv milestone done` or `hlv milestone abort` first.",
            current.id
        );
    }

    let number = map.next_number();
    let slug = format!("{:03}-{}", number, slugify(name));
    let milestone_dir = format!("human/milestones/{slug}");

    // Create directory structure
    let dirs = [
        format!("{milestone_dir}/artifacts"),
        format!("{milestone_dir}/contracts"),
        format!("{milestone_dir}/test-specs"),
    ];
    for d in &dirs {
        fs::create_dir_all(root.join(d))?;
        style::file_op("mkdir", d, None);
    }

    // Create empty plan.md
    let plan_content = format!(
        "# Milestone: {name}\n\n## Scope\n\n## Stages\n| # | Scope | Tasks | Budget | Status |\n|---|-------|-------|--------|--------|\n\n## Cross-stage dependencies\n"
    );
    let plan_path = format!("{milestone_dir}/plan.md");
    fs::write(root.join(&plan_path), plan_content)?;
    style::file_op("create", &plan_path, None);

    // Git branch creation
    let project = ProjectMap::load(&root.join("project.yaml"))?;
    let branch = if project.git.branch_per_milestone {
        let format = project
            .git
            .branch_format
            .as_deref()
            .unwrap_or("feature/{milestone-slug}");
        let branch_name = format.replace("{milestone-slug}", &slug);

        // Warn if not on main/master
        if let Ok(current_branch) = git_current_branch(root) {
            if current_branch != "main" && current_branch != "master" {
                style::warn(&format!(
                    "Currently on branch '{}', not main/master",
                    current_branch
                ));
            }
        }

        let output = Command::new("git")
            .args(["checkout", "-b", &branch_name])
            .current_dir(root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .status();

        match output {
            Ok(s) if s.success() => {
                style::file_op("branch", &branch_name, None);
                Some(branch_name)
            }
            _ => {
                style::warn(&format!(
                    "Could not create branch '{}', continuing without it",
                    branch_name
                ));
                None
            }
        }
    } else {
        None
    };

    // Update milestones.yaml
    map.current = Some(MilestoneCurrent {
        id: slug.clone(),
        number,
        branch,
        stage: None,
        stages: Vec::new(),
        gate_results: Vec::new(),
        git: None,
        labels: Vec::new(),
        meta: std::collections::HashMap::new(),
    });

    save(root, &map)?;
    style::file_op("update", "milestones.yaml", None);

    if !style::is_quiet() {
        println!();
    }
    style::ok(&format!("Milestone {} created", slug.bold()));
    style::hint("Next: run /artifacts to collect requirements, then /generate");
    if !style::is_quiet() {
        println!();
    }
    Ok(())
}

pub fn run_status(root: &Path) -> Result<()> {
    style::header("milestone status");

    let map = load(root)?;

    match &map.current {
        Some(current) => {
            println!("  {} {}", "Milestone:".bold(), current.id.bold());
            style::detail("number", &current.number.to_string());
            if let Some(branch) = &current.branch {
                style::detail("branch", branch);
            }
            if let Some(stage) = current.stage {
                style::detail("active stage", &stage.to_string());
            }

            if !current.stages.is_empty() {
                println!();
                println!("  {}", "Stages:".bold());
                for s in &current.stages {
                    let icon = match s.status {
                        StageStatus::Validated => "✓".green(),
                        StageStatus::Verified => "✓".green(),
                        StageStatus::Implementing | StageStatus::Validating => "▸".yellow(),
                        StageStatus::Implemented => "●".cyan(),
                        StageStatus::Pending => "○".dimmed(),
                    };
                    let commit_note = s
                        .commit
                        .as_ref()
                        .map(|c| format!(" ({})", &c[..7.min(c.len())]))
                        .unwrap_or_default();
                    println!(
                        "    {} Stage {}: {} [{}]{}",
                        icon,
                        s.id,
                        s.scope,
                        s.status,
                        commit_note.dimmed()
                    );
                }
            } else {
                style::hint("No stages yet. Run /generate to create them.");
            }
        }
        None => {
            style::hint("No active milestone. Run `hlv milestone new <name>` to start.");
        }
    }

    println!();
    Ok(())
}

pub fn run_list(root: &Path) -> Result<()> {
    style::header("milestone list");

    let map = load(root)?;

    if let Some(current) = &map.current {
        println!("  {} {}", "▸".yellow(), format_milestone_line(current));
    }

    if map.history.is_empty() && map.current.is_none() {
        style::hint("No milestones yet.");
    }

    for entry in map.history.iter().rev() {
        let icon = match entry.status {
            MilestoneStatus::Merged => "✓".green(),
            MilestoneStatus::Aborted => "✗".red(),
        };
        let contracts_note = if entry.contracts.is_empty() {
            String::new()
        } else {
            let names: Vec<&str> = entry.contracts.iter().map(|c| c.name.as_str()).collect();
            format!(" [{}]", names.join(", "))
        };
        let date = entry.merged_at.as_deref().unwrap_or("—").to_string();
        println!(
            "  {} {:03} {} {} {}{}",
            icon,
            entry.number,
            entry.id,
            entry.status,
            date.dimmed(),
            contracts_note.dimmed()
        );
    }

    println!();
    Ok(())
}

pub fn run_done(root: &Path) -> Result<()> {
    style::header("milestone done");

    let mut map = load(root)?;
    let current = map
        .current
        .take()
        .context("No active milestone to complete.")?;

    // Check all stages are validated
    let unfinished: Vec<String> = current
        .stages
        .iter()
        .filter(|s| s.status != StageStatus::Validated)
        .map(|s| s.scope.clone())
        .collect();

    if !unfinished.is_empty() {
        map.current = Some(current);
        anyhow::bail!(
            "Cannot complete: {} stage(s) not validated:\n  {}",
            unfinished.len(),
            unfinished.join("\n  ")
        );
    }

    // Collect contract changes (scan milestone contracts dir)
    let contracts = collect_contract_changes(root, &current.id);

    let entry = HistoryEntry {
        id: current.id.clone(),
        number: current.number,
        status: MilestoneStatus::Merged,
        contracts,
        branch: current.branch.clone(),
        merged_at: Some(chrono_today()),
    };

    // Git summary (if branch exists)
    if let Some(ref branch) = current.branch {
        if !style::is_quiet() {
            println!();
            println!("  {}", "Git summary:".bold());
        }
        // Show commit count ahead of main
        if let Ok(main_branch) = detect_main_branch(root) {
            let count = git_commit_count(root, &main_branch, branch);
            if count > 0 {
                style::detail("commits ahead", &format!("{count} (vs {main_branch})"));
            }
            // Show diffstat
            if !style::is_quiet() {
                if let Ok(diffstat) = git_diffstat(root, &main_branch) {
                    for line in diffstat.lines() {
                        println!("    {}", line.dimmed());
                    }
                }
                println!();
            }
            style::hint(&format!(
                "Merge: git checkout {main_branch} && git merge --squash {branch}"
            ));
        }
    }

    map.history.push(entry);
    save(root, &map)?;

    if !style::is_quiet() {
        println!();
    }
    style::ok(&format!("Milestone {} completed", current.id.bold()));
    if !style::is_quiet() {
        println!();
    }
    Ok(())
}

pub fn run_abort(root: &Path) -> Result<()> {
    style::header("milestone abort");

    let mut map = load(root)?;
    let current = map
        .current
        .take()
        .context("No active milestone to abort.")?;

    let entry = HistoryEntry {
        id: current.id.clone(),
        number: current.number,
        status: MilestoneStatus::Aborted,
        contracts: Vec::new(),
        branch: current.branch.clone(),
        merged_at: Some(chrono_today()),
    };

    map.history.push(entry);
    save(root, &map)?;

    if !style::is_quiet() {
        println!();
    }
    style::ok(&format!("Milestone {} aborted", current.id.bold()));
    if !style::is_quiet() {
        println!();
    }
    Ok(())
}

// ── Helpers ──────────────────────────────────────

fn milestones_path(root: &Path) -> std::path::PathBuf {
    root.join("milestones.yaml")
}

fn load(root: &Path) -> Result<MilestoneMap> {
    let path = milestones_path(root);
    anyhow::ensure!(
        path.exists(),
        "milestones.yaml not found. Run `hlv init` first."
    );
    MilestoneMap::load(&path)
}

fn load_or_create(root: &Path) -> Result<MilestoneMap> {
    let path = milestones_path(root);
    if path.exists() {
        MilestoneMap::load(&path)
    } else {
        // Read project name from project.yaml
        let project = crate::model::project::ProjectMap::load(&root.join("project.yaml"))
            .context("Cannot read project.yaml. Run `hlv init` first.")?;
        Ok(MilestoneMap {
            project: project.project,
            current: None,
            history: Vec::new(),
        })
    }
}

fn save(root: &Path, map: &MilestoneMap) -> Result<()> {
    let path = milestones_path(root);
    let content = format!(
        "# yaml-language-server: $schema=schema/milestones-schema.json\n{}",
        serde_yaml::to_string(map)?
    );
    fs::write(path, content)?;
    Ok(())
}

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn format_milestone_line(current: &MilestoneCurrent) -> String {
    let stages_done = current
        .stages
        .iter()
        .filter(|s| s.status == StageStatus::Validated)
        .count();
    let stages_total = current.stages.len();
    let stage_info = if stages_total > 0 {
        format!(" [{}/{}]", stages_done, stages_total)
    } else {
        String::new()
    };
    format!(
        "{:03} {} (active){}",
        current.number,
        current.id.bold(),
        stage_info
    )
}

fn collect_contract_changes(root: &Path, milestone_id: &str) -> Vec<ContractChange> {
    let contracts_dir = root
        .join("human/milestones")
        .join(milestone_id)
        .join("contracts");
    if !contracts_dir.exists() {
        return Vec::new();
    }
    let mut changes = Vec::new();
    if let Ok(entries) = fs::read_dir(&contracts_dir) {
        let mut seen = std::collections::HashSet::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Extract contract name from filename (e.g., order.create.md → order.create)
            let contract_name = name
                .strip_suffix(".md")
                .or_else(|| name.strip_suffix(".yaml"))
                .unwrap_or(&name)
                .to_string();
            if seen.insert(contract_name.clone()) {
                changes.push(ContractChange {
                    name: contract_name,
                    action: crate::model::milestone::ContractChangeAction::Created,
                });
            }
        }
    }
    changes
}

fn git_current_branch(root: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output()
        .context("Failed to run git")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn detect_main_branch(root: &Path) -> Result<String> {
    for candidate in &["main", "master"] {
        let status = Command::new("git")
            .args(["rev-parse", "--verify", candidate])
            .current_dir(root)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if let Ok(s) = status {
            if s.success() {
                return Ok(candidate.to_string());
            }
        }
    }
    anyhow::bail!("Neither 'main' nor 'master' branch found")
}

fn git_commit_count(root: &Path, base: &str, branch: &str) -> u32 {
    let output = Command::new("git")
        .args(["rev-list", "--count", &format!("{base}..{branch}")])
        .current_dir(root)
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .trim()
            .parse()
            .unwrap_or(0),
        Err(_) => 0,
    }
}

fn git_diffstat(root: &Path, base: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["diff", "--stat", base])
        .current_dir(root)
        .output()
        .context("Failed to run git diff")?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn chrono_today() -> String {
    // Simple date without chrono dependency
    let now = std::time::SystemTime::now();
    let since = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let days = since.as_secs() / 86400;
    // Approximate date calculation (good enough for a string)
    let year = 1970 + (days / 365);
    let remaining = days % 365;
    let month = remaining / 30 + 1;
    let day = remaining % 30 + 1;
    format!("{year:04}-{month:02}-{day:02}")
}
