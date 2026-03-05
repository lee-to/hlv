/// Parsed stage_N.md — the full context for /implement on one stage.
///
/// Format:
/// ```markdown
/// # Stage 1: Foundation (~25K)
///
/// ## Contracts
/// - order.create (this milestone)
/// - order.cancel (this milestone)
///
/// ## Tasks
///
/// TASK-001 Domain Types & Glossary
///   contracts: [order.create, order.cancel]
///   output: llm/src/domain/
///
/// TASK-002 order.create handler
///   depends_on: [TASK-001]
///   contracts: [order.create]
///   output: llm/src/features/order_create/
///
/// ## Remediation
/// (filled by /validate on failures)
/// ```
#[derive(Debug, Clone, serde::Serialize)]
pub struct StagePlan {
    pub id: u32,
    pub name: String,
    pub budget: Option<String>,
    pub contracts: Vec<String>,
    pub tasks: Vec<StageTask>,
    pub remediation: Vec<StageTask>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StageTask {
    pub id: String,
    pub name: String,
    pub contracts: Vec<String>,
    pub depends_on: Vec<String>,
    pub output: Vec<String>,
    /// Parsed from `status:` line in stage_N.md (e.g. "completed", "in_progress")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

impl StagePlan {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> anyhow::Result<Self> {
        let mut id: u32 = 0;
        let mut name = String::new();
        let mut budget: Option<String> = None;
        let mut contracts: Vec<String> = Vec::new();
        let mut tasks: Vec<StageTask> = Vec::new();
        let mut remediation: Vec<StageTask> = Vec::new();

        #[derive(PartialEq)]
        enum Section {
            None,
            Contracts,
            Tasks,
            Remediation,
        }
        let mut section = Section::None;
        let mut current_task: Option<StageTask> = None;
        let mut target = &mut tasks; // which vec to push tasks into

        for line in content.lines() {
            let trimmed = line.trim();

            // # Stage N: Name (~budget)
            if trimmed.starts_with("# Stage ") || trimmed.starts_with("# stage ") {
                if let Some(parsed) = parse_stage_header(trimmed) {
                    id = parsed.0;
                    name = parsed.1;
                    budget = parsed.2;
                }
                continue;
            }

            // ## Section headers
            if let Some(section_name) = trimmed.strip_prefix("## ") {
                // Flush current task
                if let Some(task) = current_task.take() {
                    target.push(task);
                }

                let header = section_name.trim().to_lowercase();
                if header.starts_with("contract") {
                    section = Section::Contracts;
                } else if header.starts_with("task") {
                    section = Section::Tasks;
                    target = &mut tasks;
                } else if header.starts_with("remediation") {
                    section = Section::Remediation;
                    target = &mut remediation;
                } else {
                    section = Section::None;
                }
                continue;
            }

            match section {
                Section::Contracts => {
                    // - contract.name (description)
                    if let Some(rest) = trimmed.strip_prefix("- ") {
                        let contract_name = rest
                            .split_once(' ')
                            .map(|(n, _)| n)
                            .unwrap_or(rest)
                            .to_string();
                        contracts.push(contract_name);
                    }
                }
                Section::Tasks | Section::Remediation => {
                    // TASK-NNN name or FIX-NNN name
                    if (trimmed.starts_with("TASK-") || trimmed.starts_with("FIX-"))
                        && !trimmed.starts_with(' ')
                    {
                        // Flush previous task
                        if let Some(task) = current_task.take() {
                            target.push(task);
                        }

                        let (task_id, task_name) = trimmed
                            .split_once(' ')
                            .map(|(i, n)| (i.to_string(), n.to_string()))
                            .unwrap_or_else(|| (trimmed.to_string(), String::new()));

                        current_task = Some(StageTask {
                            id: task_id,
                            name: task_name,
                            contracts: Vec::new(),
                            depends_on: Vec::new(),
                            output: Vec::new(),
                            status: None,
                        });
                    } else if let Some(ref mut task) = current_task {
                        // Task property lines (indented)
                        let prop = trimmed;
                        if let Some(val) = prop.strip_prefix("contracts:") {
                            task.contracts = parse_inline_list(val);
                        } else if let Some(val) = prop.strip_prefix("depends_on:") {
                            task.depends_on = parse_inline_list(val);
                        } else if let Some(val) = prop.strip_prefix("output:") {
                            let v = val.trim().to_string();
                            if !v.is_empty() {
                                task.output.push(v);
                            }
                        } else if let Some(val) = prop.strip_prefix("status:") {
                            let v = val.trim().to_string();
                            if !v.is_empty() {
                                task.status = Some(v);
                            }
                        }
                    }
                }
                Section::None => {}
            }
        }

        // Flush last task
        if let Some(task) = current_task.take() {
            target.push(task);
        }

        Ok(StagePlan {
            id,
            name,
            budget,
            contracts,
            tasks,
            remediation,
        })
    }

    /// All task IDs with no unmet dependencies — can run in parallel.
    pub fn ready_tasks(&self, completed: &[String]) -> Vec<&StageTask> {
        self.tasks
            .iter()
            .filter(|t| {
                !completed.contains(&t.id) && t.depends_on.iter().all(|dep| completed.contains(dep))
            })
            .collect()
    }
}

/// Parse "# Stage 2: Refund + Integration (~30K)" → (2, "Refund + Integration", Some("~30K"))
fn parse_stage_header(line: &str) -> Option<(u32, String, Option<String>)> {
    // Strip "# Stage " prefix
    let rest = line
        .strip_prefix("# Stage ")
        .or_else(|| line.strip_prefix("# stage "))?;

    // Split on ':'
    let (num_str, after_colon) = rest.split_once(':')?;
    let id: u32 = num_str.trim().parse().ok()?;

    let after = after_colon.trim();

    // Extract budget from parentheses at end
    if let Some(paren_start) = after.rfind('(') {
        if after.ends_with(')') {
            let name = after[..paren_start].trim().to_string();
            let budget = after[paren_start + 1..after.len() - 1].trim().to_string();
            return Some((id, name, Some(budget)));
        }
    }

    Some((id, after.to_string(), None))
}

/// Parse "[a, b, c]" or "a, b, c" → vec!["a", "b", "c"]
fn parse_inline_list(val: &str) -> Vec<String> {
    let cleaned = val.trim().trim_start_matches('[').trim_end_matches(']');
    cleaned
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stage_header_with_budget() {
        let r = parse_stage_header("# Stage 2: Refund + Integration (~30K)").unwrap();
        assert_eq!(r.0, 2);
        assert_eq!(r.1, "Refund + Integration");
        assert_eq!(r.2.as_deref(), Some("~30K"));
    }

    #[test]
    fn parse_stage_header_without_budget() {
        let r = parse_stage_header("# Stage 1: Foundation").unwrap();
        assert_eq!(r.0, 1);
        assert_eq!(r.1, "Foundation");
        assert!(r.2.is_none());
    }

    #[test]
    fn parse_inline_list_brackets() {
        assert_eq!(
            parse_inline_list(" [order.create, order.cancel] "),
            vec!["order.create", "order.cancel"]
        );
    }

    #[test]
    fn parse_inline_list_no_brackets() {
        assert_eq!(
            parse_inline_list(" order.create, order.cancel "),
            vec!["order.create", "order.cancel"]
        );
    }

    #[test]
    fn parse_full_stage() {
        let content = r#"# Stage 1: Foundation (~25K)

## Contracts
- order.create (this milestone)
- order.cancel (this milestone)

## Tasks

TASK-001 Domain Types & Glossary
  contracts: [order.create, order.cancel]
  output: llm/src/domain/

TASK-002 order.create handler
  depends_on: [TASK-001]
  contracts: [order.create]
  output: llm/src/features/order_create/

TASK-003 order.cancel handler
  depends_on: [TASK-001]
  contracts: [order.cancel]
  output: llm/src/features/order_cancel/

## Remediation
"#;
        let stage = StagePlan::parse(content).unwrap();
        assert_eq!(stage.id, 1);
        assert_eq!(stage.name, "Foundation");
        assert_eq!(stage.budget.as_deref(), Some("~25K"));
        assert_eq!(stage.contracts, vec!["order.create", "order.cancel"]);
        assert_eq!(stage.tasks.len(), 3);

        assert_eq!(stage.tasks[0].id, "TASK-001");
        assert_eq!(stage.tasks[0].name, "Domain Types & Glossary");
        assert!(stage.tasks[0].depends_on.is_empty());
        assert_eq!(
            stage.tasks[0].contracts,
            vec!["order.create", "order.cancel"]
        );

        assert_eq!(stage.tasks[1].id, "TASK-002");
        assert_eq!(stage.tasks[1].depends_on, vec!["TASK-001"]);

        assert!(stage.remediation.is_empty());
    }

    #[test]
    fn parse_stage_with_remediation() {
        let content = r#"# Stage 2: Core (~30K)

## Contracts
- payment.process

## Tasks

TASK-004 Payment handler
  contracts: [payment.process]
  output: llm/src/features/payment/

## Remediation

FIX-001 Fix missing error handling
  contracts: [payment.process]
  output: llm/src/features/payment/
"#;
        let stage = StagePlan::parse(content).unwrap();
        assert_eq!(stage.tasks.len(), 1);
        assert_eq!(stage.remediation.len(), 1);
        assert_eq!(stage.remediation[0].id, "FIX-001");
    }

    #[test]
    fn ready_tasks_respects_dependencies() {
        let content = r#"# Stage 1: Test (~10K)

## Tasks

TASK-001 First
  contracts: [a]
  output: llm/src/a/

TASK-002 Second
  depends_on: [TASK-001]
  contracts: [b]
  output: llm/src/b/

TASK-003 Third
  contracts: [c]
  output: llm/src/c/
"#;
        let stage = StagePlan::parse(content).unwrap();

        // Initially: TASK-001 and TASK-003 are ready (no deps)
        let ready = stage.ready_tasks(&[]);
        let ids: Vec<&str> = ready.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"TASK-001"));
        assert!(ids.contains(&"TASK-003"));
        assert!(!ids.contains(&"TASK-002"));

        // After completing TASK-001: TASK-002 becomes ready
        let ready = stage.ready_tasks(&["TASK-001".to_string()]);
        let ids: Vec<&str> = ready.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"TASK-002"));
        assert!(ids.contains(&"TASK-003"));
    }
}
