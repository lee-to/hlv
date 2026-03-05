use std::path::Path;

use anyhow::Result;

use crate::model::milestone::MilestoneMap;
use crate::model::project::{CommitConvention, ProjectMap};

pub fn run(project_root: &Path, stage_complete: bool, type_override: Option<&str>) -> Result<()> {
    let msg = get_commit_msg(project_root, stage_complete, type_override)?;
    println!("{}", msg);
    Ok(())
}

/// Returns the commit message as a String (no stdout side-effects).
pub fn get_commit_msg(
    project_root: &Path,
    stage_complete: bool,
    type_override: Option<&str>,
) -> Result<String> {
    let project = ProjectMap::load(&project_root.join("project.yaml"))?;

    let convention = &project.git.commit_convention;

    let milestones = MilestoneMap::load(&project_root.join("milestones.yaml")).ok();

    let milestone_id = milestones
        .as_ref()
        .and_then(|m| m.current.as_ref())
        .map(|c| c.id.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let (current_stage, total_stages) = milestones
        .as_ref()
        .and_then(|m| m.current.as_ref())
        .map(|c| (c.stage.unwrap_or(1), c.stages.len() as u32))
        .unwrap_or((1, 1));

    let commit_type = type_override.unwrap_or(if stage_complete { "feat" } else { "chore" });

    let stage_suffix = if total_stages > 1 {
        format!(" [stage {}/{}]", current_stage, total_stages)
    } else {
        String::new()
    };

    let msg = match convention {
        CommitConvention::Conventional => {
            format!(
                "{}({}): {}{}",
                commit_type,
                milestone_id,
                if stage_complete {
                    format!("complete stage {}", current_stage)
                } else {
                    "wip".to_string()
                },
                stage_suffix
            )
        }
        CommitConvention::Simple => {
            format!(
                "[{}] stage {}: {}",
                milestone_id,
                current_stage,
                if stage_complete { "complete" } else { "wip" }
            )
        }
        CommitConvention::Custom => {
            if let Some(ref template) = project.git.commit_template {
                template
                    .replace("{type}", commit_type)
                    .replace("{scope}", &milestone_id)
                    .replace(
                        "{message}",
                        &if stage_complete {
                            format!("complete stage {}", current_stage)
                        } else {
                            "wip".to_string()
                        },
                    )
            } else {
                format!(
                    "{}({}): {}{}",
                    commit_type,
                    milestone_id,
                    if stage_complete { "complete" } else { "wip" },
                    stage_suffix
                )
            }
        }
    };

    Ok(msg)
}
