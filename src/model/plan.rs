/// Plan is parsed from human/milestones/{id}/plan.md (Markdown).
/// This struct represents the parsed result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanMd {
    pub generated_from: Option<String>,
    pub date: Option<String>,
    pub total_tasks: Option<u32>,
    pub parallel_groups: Option<u32>,
    pub overview: String,
    pub groups: Vec<PlanMdGroup>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanMdGroup {
    pub number: u32,
    pub name: String,
    pub tasks: Vec<PlanMdTask>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanMdTask {
    pub id: String,
    pub subject: String,
    pub scope: Option<String>,
    pub contracts: Vec<String>,
    pub depends_on: Vec<String>,
    pub agent_slot: Option<String>,
    pub output: Vec<String>,
}

impl PlanMd {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::parse(&content))
    }

    pub fn parse(content: &str) -> Self {
        let mut generated_from: Option<String> = None;
        let mut date: Option<String> = None;
        let mut overview = String::new();
        let mut groups: Vec<PlanMdGroup> = Vec::new();

        #[derive(PartialEq)]
        enum Section {
            None,
            Scope,
            Stages,
            Other,
        }
        let mut section = Section::None;
        let mut other_text = String::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // # Milestone: name (top-level title -> overview)
            if trimmed.starts_with("# ") && !trimmed.starts_with("## ") {
                // Extract metadata from header if present
                if let Some(rest) = trimmed.strip_prefix("# Milestone: ") {
                    overview = rest.to_string();
                } else {
                    overview = trimmed.strip_prefix("# ").unwrap_or(trimmed).to_string();
                }
                continue;
            }

            // ## Section headers
            if let Some(header) = trimmed.strip_prefix("## ") {
                let h = header.trim().to_lowercase();
                if h == "scope" || h == "overview" {
                    section = Section::Scope;
                } else if h.starts_with("stage") {
                    section = Section::Stages;
                } else {
                    // Capture other sections as overview text
                    section = Section::Other;
                    other_text.push_str(&format!("\n{header}\n"));
                }
                continue;
            }

            // Metadata lines: generated_from: ..., date: ...
            if let Some(val) = trimmed.strip_prefix("generated_from:") {
                generated_from = Some(val.trim().to_string());
                continue;
            }
            if let Some(val) = trimmed.strip_prefix("date:") {
                date = Some(val.trim().to_string());
                continue;
            }

            match section {
                Section::Scope => {
                    if !trimmed.is_empty() {
                        if !overview.is_empty() {
                            overview.push('\n');
                        }
                        overview.push_str(trimmed);
                    }
                }
                Section::Stages => {
                    // Parse markdown table rows: | 1 | Scope text | 4 | ~25K | pending |
                    if trimmed.starts_with('|') && trimmed.ends_with('|') {
                        let inner = &trimmed[1..trimmed.len() - 1];
                        // Skip separator and header rows
                        if inner
                            .chars()
                            .all(|c| c == '-' || c == '|' || c == ' ' || c == ':')
                        {
                            continue;
                        }
                        let cells: Vec<&str> = inner.split('|').map(|c| c.trim()).collect();
                        if cells.len() >= 2 {
                            // Skip header row
                            if cells[0] == "#" || cells[0].to_lowercase() == "stage" {
                                continue;
                            }
                            if let Ok(num) = cells[0].parse::<u32>() {
                                let name = cells.get(1).unwrap_or(&"").to_string();
                                groups.push(PlanMdGroup {
                                    number: num,
                                    name,
                                    tasks: Vec::new(),
                                });
                            }
                        }
                    }
                }
                Section::Other => {
                    if !trimmed.is_empty() {
                        other_text.push_str(trimmed);
                        other_text.push('\n');
                    }
                }
                Section::None => {}
            }
        }

        // Append other sections to overview if present
        if !other_text.is_empty() {
            if !overview.is_empty() {
                overview.push('\n');
            }
            overview.push_str(other_text.trim());
        }

        PlanMd {
            generated_from,
            date,
            total_tasks: None,
            parallel_groups: if groups.is_empty() {
                None
            } else {
                Some(groups.len() as u32)
            },
            overview,
            groups,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_plan() {
        let content = r#"# Milestone: checkout

## Scope
Commerce checkout: order creation and cancellation with full validation pipeline.

## Stages
| # | Scope | Tasks | Budget | Status |
|---|-------|-------|--------|--------|
| 1 | Domain types + order.create + order.cancel | 4 | ~25K | implementing |
| 2 | Integration tests + observability | 2 | ~20K | pending |

## Cross-stage dependencies
Stage 2 uses types and handlers from Stage 1
"#;
        let plan = PlanMd::parse(content);
        assert_eq!(plan.overview, "checkout\nCommerce checkout: order creation and cancellation with full validation pipeline.\nCross-stage dependencies\nStage 2 uses types and handlers from Stage 1");
        assert_eq!(plan.groups.len(), 2);
        assert_eq!(plan.groups[0].number, 1);
        assert_eq!(
            plan.groups[0].name,
            "Domain types + order.create + order.cancel"
        );
        assert_eq!(plan.groups[1].number, 2);
        assert_eq!(plan.parallel_groups, Some(2));
    }

    #[test]
    fn parse_empty_plan() {
        let plan = PlanMd::parse("");
        assert!(plan.overview.is_empty());
        assert!(plan.groups.is_empty());
        assert!(plan.parallel_groups.is_none());
    }
}
