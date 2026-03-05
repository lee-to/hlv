use crate::parse::markdown;

/// A contract parsed from a Markdown file (human/milestones/{id}/contracts/*.md).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContractMd {
    pub id: String,
    pub version: String,
    pub owner: Option<String>,
    pub sources: Vec<String>,
    pub intent: String,
    pub input_yaml: Option<String>,
    pub output_yaml: Option<String>,
    pub errors: Vec<ErrorRow>,
    pub invariants: Vec<String>,
    pub examples: Vec<Example>,
    pub edge_cases: Vec<String>,
    pub nfr_yaml: Option<String>,
    pub security: Vec<String>,
    /// Raw sections for extensibility
    pub sections: Vec<(String, String)>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorRow {
    pub code: String,
    pub http_status: String,
    pub when: String,
    pub source: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Example {
    pub title: String,
    pub json_blocks: Vec<String>,
}

impl ContractMd {
    /// Parse a contract from raw markdown text.
    pub fn from_markdown(text: &str) -> Self {
        let (id, version, owner) = markdown::parse_header(text);
        let sections = markdown::extract_sections(text);

        let mut sources = Vec::new();
        let mut intent = String::new();
        let mut input_yaml = None;
        let mut output_yaml = None;
        let mut errors = Vec::new();
        let mut invariants = Vec::new();
        let mut examples = Vec::new();
        let mut edge_cases = Vec::new();
        let mut nfr_yaml = None;
        let mut security = Vec::new();
        let mut raw_sections = Vec::new();

        for section in &sections {
            let title_lower = section.title.to_lowercase();
            raw_sections.push((section.title.clone(), section.body.clone()));

            if title_lower == "sources" {
                // Extract links from body lines starting with "- "
                for line in section.body.lines() {
                    let trimmed = line.trim().trim_start_matches("- ");
                    if !trimmed.is_empty() {
                        sources.push(trimmed.to_string());
                    }
                }
            } else if title_lower == "intent" {
                intent = section.body.clone();
            } else if title_lower == "input" {
                let yaml_blocks = markdown::extract_yaml_blocks_in_section(text, &section.title);
                if let Some(first) = yaml_blocks.first() {
                    input_yaml = Some(first.clone());
                }
            } else if title_lower == "output" {
                let yaml_blocks = markdown::extract_yaml_blocks_in_section(text, &section.title);
                if let Some(first) = yaml_blocks.first() {
                    output_yaml = Some(first.clone());
                }
            } else if title_lower == "errors" {
                // Parse error table from raw text
                let error_table = parse_error_table_from_raw(text);
                errors = error_table;
            } else if title_lower == "invariants" {
                // Extract invariant items — look for bold names or numbered items
                for line in section.body.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('>') {
                        continue;
                    }
                    if trimmed.starts_with("**")
                        || trimmed.starts_with("- **")
                        || trimmed
                            .chars()
                            .next()
                            .map(|c| c.is_ascii_digit())
                            .unwrap_or(false)
                    {
                        invariants.push(trimmed.to_string());
                    }
                }
                // Fallback: if pulldown-cmark flattened the structure, grab non-empty lines
                if invariants.is_empty() {
                    for line in section.body.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() && !trimmed.starts_with('>') {
                            invariants.push(trimmed.to_string());
                        }
                    }
                }
            } else if title_lower == "examples" {
                // Extract examples from subsections
                let json_blocks = markdown::extract_json_blocks(text);
                // Group by subsection headers
                let mut current_example_title = String::from("Example");
                let mut current_blocks: Vec<String> = Vec::new();
                let mut block_idx = 0;

                for line in section.body.lines() {
                    let trimmed = line.trim();
                    if let Some(heading) = trimmed.strip_prefix("### ") {
                        if !current_blocks.is_empty() {
                            examples.push(Example {
                                title: current_example_title.clone(),
                                json_blocks: current_blocks.clone(),
                            });
                            current_blocks.clear();
                        }
                        current_example_title = heading.to_string();
                    } else if trimmed.contains("```json") && block_idx < json_blocks.len() {
                        current_blocks.push(json_blocks[block_idx].clone());
                        block_idx += 1;
                    }
                }
                if !current_blocks.is_empty() {
                    examples.push(Example {
                        title: current_example_title,
                        json_blocks: current_blocks,
                    });
                }
                // Fallback: if we didn't find subsections, group all json blocks
                if examples.is_empty() && !json_blocks.is_empty() {
                    examples.push(Example {
                        title: "Examples".to_string(),
                        json_blocks,
                    });
                }
            } else if title_lower == "edge cases" {
                for line in section.body.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('>') {
                        continue;
                    }
                    if trimmed.starts_with("**")
                        || trimmed.starts_with("- **")
                        || trimmed
                            .chars()
                            .next()
                            .map(|c| c.is_ascii_digit())
                            .unwrap_or(false)
                    {
                        edge_cases.push(trimmed.to_string());
                    }
                }
                if edge_cases.is_empty() {
                    for line in section.body.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() && !trimmed.starts_with('>') {
                            edge_cases.push(trimmed.to_string());
                        }
                    }
                }
            } else if title_lower == "nfr" {
                let yaml_blocks = markdown::extract_yaml_blocks_in_section(text, &section.title);
                if let Some(first) = yaml_blocks.first() {
                    nfr_yaml = Some(first.clone());
                }
            } else if title_lower == "security" {
                for line in section.body.lines() {
                    let trimmed = line.trim().trim_start_matches("- ");
                    if !trimmed.is_empty() {
                        security.push(trimmed.to_string());
                    }
                }
            }
        }

        ContractMd {
            id,
            version,
            owner,
            sources,
            intent,
            input_yaml,
            output_yaml,
            errors,
            invariants,
            examples,
            edge_cases,
            nfr_yaml,
            security,
            sections: raw_sections,
        }
    }

    /// List of mandatory section names.
    pub fn required_sections() -> &'static [&'static str] {
        &[
            "Sources",
            "Intent",
            "Input",
            "Output",
            "Errors",
            "Invariants",
            "Examples",
            "NFR",
            "Security",
        ]
    }

    /// Check which required sections are present.
    pub fn present_section_names(&self) -> Vec<String> {
        self.sections.iter().map(|(t, _)| t.clone()).collect()
    }

    pub fn has_happy_path_example(&self) -> bool {
        self.examples.iter().any(|e| {
            let t = e.title.to_lowercase();
            t.contains("happy") || t.contains("success")
        })
    }

    pub fn has_error_example(&self) -> bool {
        self.examples.iter().any(|e| {
            let t = e.title.to_lowercase();
            t.contains("error")
                || t.contains("not found")
                || t.contains("out of stock")
                || t.contains("forbidden")
                || t.contains("not cancellable")
        })
    }
}

/// Parse error table directly from raw markdown text.
fn parse_error_table_from_raw(text: &str) -> Vec<ErrorRow> {
    let mut rows = Vec::new();
    let mut in_errors = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "## Errors" {
            in_errors = true;
            continue;
        }
        if in_errors && trimmed.starts_with("## ") {
            break;
        }
        if !in_errors {
            continue;
        }
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            continue;
        }
        let inner = &trimmed[1..trimmed.len() - 1];
        // Skip header and separator
        if inner
            .chars()
            .all(|c| c == '-' || c == '|' || c == ' ' || c == ':')
        {
            continue;
        }
        let cells: Vec<String> = inner.split('|').map(|c| c.trim().to_string()).collect();
        if cells.len() >= 4 {
            // Skip the header row
            if cells[0] == "Code" {
                continue;
            }
            rows.push(ErrorRow {
                code: cells[0].clone(),
                http_status: cells[1].clone(),
                when: cells[2].clone(),
                source: cells[3].clone(),
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "tests/fixtures/example-project";

    #[test]
    fn test_parse_order_create_md() {
        let text = std::fs::read_to_string(format!(
            "{FIXTURE}/human/milestones/001/contracts/order.create.md"
        ))
        .unwrap();
        let c = ContractMd::from_markdown(&text);
        assert_eq!(c.id, "order.create");
        assert_eq!(c.version, "1.2.0");
        assert_eq!(c.owner.as_deref(), Some("commerce"));
        assert!(!c.sources.is_empty());
        assert!(!c.intent.is_empty());
        assert!(c.input_yaml.is_some());
        assert!(c.output_yaml.is_some());
        assert!(!c.errors.is_empty(), "errors should be parsed");
        assert!(!c.invariants.is_empty(), "invariants should be parsed");
        assert!(!c.examples.is_empty(), "examples should be parsed");
        assert!(c.nfr_yaml.is_some());
        assert!(!c.security.is_empty());
    }

    #[test]
    fn test_parse_order_cancel_md() {
        let text = std::fs::read_to_string(format!(
            "{FIXTURE}/human/milestones/001/contracts/order.cancel.md"
        ))
        .unwrap();
        let c = ContractMd::from_markdown(&text);
        assert_eq!(c.id, "order.cancel");
        assert_eq!(c.version, "1.0.0");
        assert!(!c.errors.is_empty());
        assert!(!c.invariants.is_empty());
        assert!(!c.examples.is_empty());
    }
}
