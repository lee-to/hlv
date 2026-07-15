use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use regex::Regex;

/// A section extracted from Markdown, split on `## ` headings.
#[derive(Debug, Clone)]
pub struct Section {
    pub title: String,
    pub level: u32,
    pub body: String,
}

/// Extract sections from Markdown split on headings.
/// Level 2 (`##`) sections become top-level sections.
/// Level 3 (`###`) subsections are included in the body of their parent.
pub fn extract_sections(md: &str) -> Vec<Section> {
    let parser = Parser::new(md);
    let mut sections: Vec<Section> = Vec::new();
    let mut current_title = String::new();
    let mut current_level: u32 = 0;
    let mut current_body = String::new();
    let mut in_heading = false;
    let mut heading_text = String::new();
    let mut heading_level: u32 = 0;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = true;
                heading_text.clear();
                heading_level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                if heading_level == 2 {
                    // Save previous section
                    if !current_title.is_empty() || !current_body.trim().is_empty() {
                        sections.push(Section {
                            title: current_title.clone(),
                            level: current_level,
                            body: current_body.trim().to_string(),
                        });
                    }
                    current_title = heading_text.clone();
                    current_level = heading_level;
                    current_body.clear();
                } else if heading_level == 1 {
                    // Store as preamble section
                    if !current_title.is_empty() || !current_body.trim().is_empty() {
                        sections.push(Section {
                            title: current_title.clone(),
                            level: current_level,
                            body: current_body.trim().to_string(),
                        });
                    }
                    current_title = heading_text.clone();
                    current_level = heading_level;
                    current_body.clear();
                } else {
                    // ### subsections go into body as markdown
                    let prefix = "#".repeat(heading_level as usize);
                    current_body.push_str(&format!("\n{} {}\n\n", prefix, heading_text));
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if in_heading {
                    heading_text.push_str(&text);
                } else {
                    current_body.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak if !in_heading => {
                current_body.push('\n');
            }
            Event::SoftBreak | Event::HardBreak => {}
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match &kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                    _ => String::new(),
                };
                current_body.push_str(&format!("\n```{}\n", lang));
            }
            Event::End(TagEnd::CodeBlock) => {
                current_body.push_str("```\n");
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                current_body.push('\n');
            }
            Event::Start(Tag::List(_)) => {}
            Event::End(TagEnd::List(_)) => {
                current_body.push('\n');
            }
            Event::Start(Tag::Item) => {
                current_body.push_str("- ");
            }
            Event::End(TagEnd::Item) => {
                current_body.push('\n');
            }
            Event::Start(Tag::Table(_)) => {
                current_body.push_str("\n|TABLE_START|\n");
            }
            Event::End(TagEnd::Table) => {
                current_body.push_str("|TABLE_END|\n");
            }
            Event::Start(Tag::TableHead) => {
                current_body.push('|');
            }
            Event::End(TagEnd::TableHead) => {
                current_body.push('\n');
            }
            Event::Start(Tag::TableRow) => {
                current_body.push('|');
            }
            Event::End(TagEnd::TableRow) => {
                current_body.push('\n');
            }
            Event::Start(Tag::TableCell) => {}
            Event::End(TagEnd::TableCell) => {
                current_body.push('|');
            }
            Event::Start(Tag::BlockQuote(_)) => {
                current_body.push_str("> ");
            }
            Event::Start(Tag::Strong) | Event::End(TagEnd::Strong) => {
                current_body.push_str("**");
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                current_body.push('[');
                // We'll capture the text, then close with dest
                // Actually let's just push a marker
                current_body.push_str(&format!("__LINK_DEST:{}__", dest_url));
            }
            Event::End(TagEnd::Link) => {
                current_body.push(']');
            }
            _ => {}
        }
    }

    // Save last section
    if !current_title.is_empty() || !current_body.trim().is_empty() {
        sections.push(Section {
            title: current_title,
            level: current_level,
            body: current_body.trim().to_string(),
        });
    }

    sections
}

/// Extract declared test IDs from accepted test-spec shapes:
/// `### CT-...:`, bullet rows beginning with an ID, and Markdown tables
/// where the first cell is the ID.
pub fn extract_test_ids(md: &str) -> Vec<String> {
    extract_test_ids_with_pattern(md, None)
}

/// Extract test IDs while accepting an additional project-specific pattern.
/// Built-in HLV test prefixes are always recognized for backward compatibility.
pub fn extract_test_ids_with_pattern(md: &str, additional_pattern: Option<&Regex>) -> Vec<String> {
    let mut ids = Vec::new();

    for section in extract_sections(md) {
        for line in section.body.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("### ") {
                if let Some(id) =
                    first_test_id_token(trimmed.trim_start_matches('#').trim(), additional_pattern)
                {
                    ids.push(id);
                }
            } else if let Some(item) = markdown_list_item(trimmed) {
                if let Some(id) = first_test_id_token(item, additional_pattern) {
                    ids.push(id);
                }
            } else if trimmed.starts_with('|') {
                let first_cell = trimmed
                    .trim_matches('|')
                    .split('|')
                    .next()
                    .unwrap_or("")
                    .trim();
                if let Some(id) = first_test_id_token(first_cell, additional_pattern) {
                    ids.push(id);
                }
            }
        }
    }

    ids
}

fn markdown_list_item(line: &str) -> Option<&str> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return Some(rest.trim());
        }
    }

    let (number, rest) = line.split_once(". ")?;
    if !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) {
        Some(rest.trim())
    } else {
        None
    }
}

fn first_test_id_token(text: &str, additional_pattern: Option<&Regex>) -> Option<String> {
    let text = text.trim().trim_start_matches(['*', '`', '[', '(']);

    if let Some(candidate) = additional_pattern
        .and_then(|pattern| longest_pattern_match_at_token_boundary(text, pattern))
    {
        return Some(candidate);
    }

    let candidate = text
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(*c, '-' | '_' | '.'))
        .collect::<String>();

    let prefixes = ["CT-", "PBT-", "IT-", "EC-", "PERF-", "SEC-", "TST-"];
    let matches_builtin = prefixes
        .iter()
        .any(|prefix| candidate.starts_with(prefix) && candidate.len() > prefix.len());

    if matches_builtin {
        Some(candidate)
    } else {
        None
    }
}

fn longest_pattern_match_at_token_boundary(text: &str, pattern: &Regex) -> Option<String> {
    let mut boundaries: Vec<usize> = text.char_indices().map(|(index, _)| index).collect();
    boundaries.push(text.len());

    boundaries.into_iter().rev().find_map(|end| {
        if end == 0 || !is_test_id_token_boundary(text[end..].chars().next()) {
            return None;
        }

        let candidate = &text[..end];
        pattern
            .find(candidate)
            .filter(|matched| matched.start() == 0 && matched.end() == candidate.len())
            .map(|_| candidate.to_string())
    })
}

fn is_test_id_token_boundary(next: Option<char>) -> bool {
    match next {
        None => true,
        Some(character) => {
            character.is_whitespace()
                || matches!(character, ':' | '`' | '*' | ']' | ')' | ',' | ';' | '|')
        }
    }
}

/// Extract ```yaml ... ``` code blocks from a markdown text (raw string, not parsed).
pub fn extract_yaml_blocks(text: &str) -> Vec<String> {
    extract_fenced_blocks(text, "yaml")
}

/// Extract ```json ... ``` code blocks from a markdown text.
pub fn extract_json_blocks(text: &str) -> Vec<String> {
    extract_fenced_blocks(text, "json")
}

/// Extract the raw text of a `## <title>` section (from heading to next `## ` or EOF).
pub fn extract_section_raw<'a>(text: &'a str, section_title: &str) -> Option<&'a str> {
    let target = format!("## {}", section_title);
    let mut start = None;
    let mut byte_pos = 0;
    let mut line_offsets: Vec<usize> = Vec::new(); // byte offset of each line start

    // Pre-compute line start offsets to handle both \n and \r\n
    for line in text.split('\n') {
        line_offsets.push(byte_pos);
        byte_pos += line.len() + 1; // +1 for the '\n'
    }

    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed == target || trimmed.starts_with(&format!("{}  ", target)) {
            let byte_start = if idx + 1 < line_offsets.len() {
                line_offsets[idx + 1]
            } else {
                text.len()
            };
            start = Some(byte_start.min(text.len()));
        } else if start.is_some() && trimmed.starts_with("## ") {
            let byte_end = line_offsets[idx];
            return Some(&text[start.unwrap()..byte_end.min(text.len())]);
        }
    }
    start.map(|s| &text[s..])
}

/// Extract YAML blocks only within a specific `## <title>` section.
pub fn extract_yaml_blocks_in_section(text: &str, section_title: &str) -> Vec<String> {
    match extract_section_raw(text, section_title) {
        Some(section_text) => extract_fenced_blocks(section_text, "yaml"),
        None => Vec::new(),
    }
}

fn extract_fenced_blocks(text: &str, lang: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut current = String::new();

    for line in text.lines() {
        if !in_block {
            let trimmed = line.trim();
            if trimmed.starts_with("```") && trimmed[3..].trim().starts_with(lang) {
                in_block = true;
                current.clear();
            }
        } else if line.trim().starts_with("```") {
            blocks.push(current.trim().to_string());
            current.clear();
            in_block = false;
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }

    blocks
}

/// Extract table rows from raw markdown.
/// Returns rows as Vec<Vec<String>>, skipping the separator row (---|---).
pub fn extract_table_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            continue;
        }
        // Skip separator rows
        let inner = &trimmed[1..trimmed.len() - 1];
        if inner
            .chars()
            .all(|c| c == '-' || c == '|' || c == ' ' || c == ':')
        {
            continue;
        }
        let cells: Vec<String> = inner.split('|').map(|c| c.trim().to_string()).collect();
        rows.push(cells);
    }
    rows
}

/// Parse the header of a contract MD file.
/// Expected format: `# contract.id v1.0.0` followed by `owner: team-name`.
/// Returns (id, version, owner)
pub fn parse_header(md: &str) -> (String, String, Option<String>) {
    let mut id = String::new();
    let mut version = String::new();
    let mut owner: Option<String> = None;

    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") && id.is_empty() {
            let rest = &trimmed[2..];
            // Split "order.create v1.0.0" -> id + version
            let parts: Vec<&str> = rest.splitn(2, ' ').collect();
            if let Some(first) = parts.first() {
                id = first.to_string();
            }
            if let Some(second) = parts.get(1) {
                version = second.trim_start_matches('v').to_string();
            }
        } else if let Some(rest) = trimmed.strip_prefix("owner:") {
            owner = Some(rest.trim().to_string());
        } else if !trimmed.is_empty() && !id.is_empty() {
            // Stop after header block
            break;
        }
    }

    (id, version, owner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_integration_test_id_from_heading() {
        let md = "## Integration Tests\n\n### IT-CHECKOUT-001: Complete checkout\n";

        assert_eq!(extract_test_ids(md), vec!["IT-CHECKOUT-001"]);
    }

    #[test]
    fn test_extract_integration_test_id_from_bullet() {
        let md = "## Integration Tests\n\n- **IT-CHECKOUT-001** Complete checkout\n";

        assert_eq!(extract_test_ids(md), vec!["IT-CHECKOUT-001"]);
    }

    #[test]
    fn test_extract_project_specific_test_id_with_pattern() {
        let md = "## Integration Tests\n\n### INT_CHECKOUT_001: Complete checkout\n";
        let pattern = Regex::new(r"^INT_[A-Z]+_[0-9]{3}$").unwrap();

        assert_eq!(
            extract_test_ids_with_pattern(md, Some(&pattern)),
            vec!["INT_CHECKOUT_001"]
        );
        assert!(extract_test_ids(md).is_empty());
    }

    #[test]
    fn test_extract_project_specific_test_id_with_punctuation() {
        let md = "## Integration Tests\n\n### QA/123: Complete checkout\n";
        let pattern = Regex::new(r"^QA/[0-9]+$").unwrap();

        assert_eq!(
            extract_test_ids_with_pattern(md, Some(&pattern)),
            vec!["QA/123"]
        );
    }

    #[test]
    fn test_extract_yaml_blocks() {
        let md = r#"Some text

```yaml
type: object
required: [user_id]
```

more text

```yaml
another: block
```
"#;
        let blocks = extract_yaml_blocks(md);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("type: object"));
        assert!(blocks[1].contains("another: block"));
    }

    #[test]
    fn test_extract_json_blocks() {
        let md = r#"
```json
{"key": "value"}
```
"#;
        let blocks = extract_json_blocks(md);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn test_parse_header() {
        let md = "# order.create v1.0.0\nowner: commerce\n\n## Sources\n";
        let (id, ver, owner) = parse_header(md);
        assert_eq!(id, "order.create");
        assert_eq!(ver, "1.0.0");
        assert_eq!(owner.as_deref(), Some("commerce"));
    }

    #[test]
    fn test_extract_table_rows() {
        let md = r#"| Code | HTTP | When |
|------|------|------|
| OUT_OF_STOCK | 409 | stock low |
| NOT_FOUND | 404 | missing |"#;
        let rows = extract_table_rows(md);
        assert_eq!(rows.len(), 3); // header + 2 data rows
        assert_eq!(rows[0][0], "Code");
        assert_eq!(rows[1][0], "OUT_OF_STOCK");
    }

    #[test]
    fn test_extract_yaml_unterminated_block() {
        let md = "```yaml\ntype: object\nno closing fence here\n";
        let blocks = extract_yaml_blocks(md);
        assert!(
            blocks.is_empty(),
            "unterminated block should not be extracted"
        );
    }

    #[test]
    fn test_parse_header_no_version() {
        let md = "# mycontract\nowner: team\n\n## Sources\n";
        let (id, ver, owner) = parse_header(md);
        assert_eq!(id, "mycontract");
        assert_eq!(ver, "");
        assert_eq!(owner.as_deref(), Some("team"));
    }

    #[test]
    fn test_parse_header_no_owner() {
        let md = "# order.create v2.0.0\n\n## Intent\n";
        let (id, ver, owner) = parse_header(md);
        assert_eq!(id, "order.create");
        assert_eq!(ver, "2.0.0");
        assert!(owner.is_none());
    }

    #[test]
    fn test_parse_header_empty() {
        let (id, ver, owner) = parse_header("");
        assert_eq!(id, "");
        assert_eq!(ver, "");
        assert!(owner.is_none());
    }

    #[test]
    fn test_extract_sections() {
        let md = r#"# order.create v1.0.0
owner: commerce

## Sources

- link1
- link2

## Intent

Create an order.

## Input

```yaml
type: object
```
"#;
        let sections = extract_sections(md);
        assert!(sections.len() >= 3);
        assert_eq!(sections[0].title, "order.create v1.0.0");
        assert_eq!(sections[1].title, "Sources");
        assert_eq!(sections[2].title, "Intent");
    }

    #[test]
    fn test_extract_sections_with_subsections() {
        let md = "## Parent\n\nSome text.\n\n### Child\n\nChild body.\n\n## Next\n\nMore.\n";
        let sections = extract_sections(md);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "Parent");
        assert!(sections[0].body.contains("Child"));
        assert_eq!(sections[1].title, "Next");
    }

    #[test]
    fn test_extract_sections_empty() {
        let sections = extract_sections("");
        assert!(sections.is_empty());
    }

    #[test]
    fn test_extract_sections_only_body_no_heading() {
        let md = "Just some text without headings.\n";
        let sections = extract_sections(md);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "");
    }

    #[test]
    fn test_extract_section_raw_found() {
        let md = "## Intent\n\nCreate an order.\n\n## Errors\n\nSome errors.\n";
        let section = extract_section_raw(md, "Intent");
        assert!(section.is_some());
        assert!(section.unwrap().contains("Create an order."));
        assert!(!section.unwrap().contains("Some errors."));
    }

    #[test]
    fn test_extract_section_raw_not_found() {
        let md = "## Intent\n\nCreate an order.\n";
        assert!(extract_section_raw(md, "Missing").is_none());
    }

    #[test]
    fn test_extract_section_raw_last_section() {
        let md = "## First\n\nAAA\n\n## Last\n\nBBB\n";
        let section = extract_section_raw(md, "Last");
        assert!(section.is_some());
        assert!(section.unwrap().contains("BBB"));
    }

    #[test]
    fn test_extract_yaml_blocks_in_section_found() {
        let md =
            "## Input\n\n```yaml\ntype: object\n```\n\n## Output\n\n```yaml\ntype: string\n```\n";
        let blocks = extract_yaml_blocks_in_section(md, "Input");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("type: object"));
    }

    #[test]
    fn test_extract_yaml_blocks_in_section_not_found() {
        let md = "## Input\n\n```yaml\ntype: object\n```\n";
        let blocks = extract_yaml_blocks_in_section(md, "Missing");
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_extract_json_blocks_multiple() {
        let md = "```json\n{\"a\": 1}\n```\n\n```json\n{\"b\": 2}\n```\n";
        let blocks = extract_json_blocks(md);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("\"a\""));
        assert!(blocks[1].contains("\"b\""));
    }

    #[test]
    fn test_extract_json_blocks_empty() {
        let blocks = extract_json_blocks("no json here\n");
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_extract_table_rows_empty() {
        let rows = extract_table_rows("no table here\n");
        assert!(rows.is_empty());
    }

    #[test]
    fn test_extract_table_rows_separator_only() {
        let md = "|---|---|\n";
        let rows = extract_table_rows(md);
        assert!(rows.is_empty());
    }

    #[test]
    fn test_extract_table_rows_with_colons() {
        let md = "| A | B |\n|:---:|:---:|\n| 1 | 2 |\n";
        let rows = extract_table_rows(md);
        assert_eq!(rows.len(), 2); // header + data
        assert_eq!(rows[1][0], "1");
    }

    #[test]
    fn test_extract_yaml_blocks_ignores_other_langs() {
        let md = "```rust\nfn main() {}\n```\n\n```yaml\nkey: val\n```\n";
        let blocks = extract_yaml_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("key: val"));
    }
}
