use std::collections::{HashMap, HashSet};
use std::path::Path;

use petgraph::algo::is_cyclic_directed;
use petgraph::graph::DiGraph;

use crate::check::Diagnostic;
use crate::model::project::ContractEntry;
use crate::model::stage::StagePlan;

/// Validate stage_N.md files for a milestone.
pub fn check_stage_plans(
    root: &Path,
    milestone_id: &str,
    contracts: &[ContractEntry],
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let ms_dir = root.join("human/milestones").join(milestone_id);

    // Find all stage_N.md files
    let mut stage_files: Vec<(u32, std::path::PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&ms_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(rest) = name.strip_prefix("stage_") {
                if let Some(num_str) = rest.strip_suffix(".md") {
                    if let Ok(n) = num_str.parse::<u32>() {
                        stage_files.push((n, entry.path()));
                    }
                }
            }
        }
    }

    stage_files.sort_by_key(|(n, _)| *n);

    if stage_files.is_empty() {
        diags.push(Diagnostic::info("PLN-001", "No stage files found"));
        return diags;
    }

    let contract_ids: HashSet<&str> = contracts.iter().map(|c| c.id.as_str()).collect();
    let mut all_covered: HashSet<String> = HashSet::new();
    let mut all_task_ids: HashSet<String> = HashSet::new();

    for (stage_num, path) in &stage_files {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => {
                diags.push(Diagnostic::error(
                    "PLN-010",
                    format!("Cannot read stage_{}.md", stage_num),
                ));
                continue;
            }
        };

        let stage = match StagePlan::parse(&content) {
            Ok(s) => s,
            Err(e) => {
                diags.push(Diagnostic::error(
                    "PLN-010",
                    format!("Cannot parse stage_{}.md: {}", stage_num, e),
                ));
                continue;
            }
        };

        // Check task ID uniqueness across all stages
        for task in &stage.tasks {
            if !all_task_ids.insert(task.id.clone()) {
                diags.push(Diagnostic::error(
                    "PLN-010",
                    format!("Duplicate task ID across stages: {}", task.id),
                ));
            }
            // Collect covered contracts
            for c in &task.contracts {
                all_covered.insert(c.clone());
            }
        }

        // Check intra-stage dependency graph for cycles
        let stage_task_ids: HashSet<&str> = stage.tasks.iter().map(|t| t.id.as_str()).collect();
        let mut graph = DiGraph::<&str, ()>::new();
        let mut node_map: HashMap<&str, petgraph::graph::NodeIndex> = HashMap::new();

        for task in &stage.tasks {
            let idx = graph.add_node(task.id.as_str());
            node_map.insert(task.id.as_str(), idx);
        }

        for task in &stage.tasks {
            for dep in &task.depends_on {
                if stage_task_ids.contains(dep.as_str()) {
                    if let (Some(&from), Some(&to)) =
                        (node_map.get(dep.as_str()), node_map.get(task.id.as_str()))
                    {
                        graph.add_edge(from, to, ());
                    }
                }
                // Cross-stage deps are allowed (referencing tasks from previous stages)
            }
        }

        if is_cyclic_directed(&graph) {
            diags.push(Diagnostic::error(
                "PLN-020",
                format!("Stage {} contains a dependency cycle", stage_num),
            ));
        }
    }

    // Check contract coverage across all stages
    for cid in &contract_ids {
        if !all_covered.contains(*cid) {
            diags.push(Diagnostic::warning(
                "PLN-040",
                format!("Contract '{}' not covered by any task in any stage", cid),
            ));
        }
    }

    diags
}
