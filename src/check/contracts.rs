use std::path::Path;

use crate::check::Diagnostic;
use crate::model::contract_md::ContractMd;
use crate::model::contract_yaml::ContractYaml;
use crate::model::glossary::Glossary;
use crate::model::project::ContractEntry;

/// Validate all contracts (MD + YAML) for a project.
pub fn check_contracts(
    root: &Path,
    entries: &[ContractEntry],
    glossary: &Glossary,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    for entry in entries {
        diags.extend(check_contract_md(root, entry, glossary));
        if let Some(ref yp) = entry.yaml_path {
            diags.extend(check_contract_yaml(root, entry, yp));
        }
    }

    diags
}

fn check_contract_md(root: &Path, entry: &ContractEntry, glossary: &Glossary) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let path = root.join(&entry.path);
    let file_label = &entry.path;

    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            diags.push(
                Diagnostic::error("CTR-001", format!("Cannot read contract: {}", e))
                    .with_file(file_label),
            );
            return diags;
        }
    };

    let contract = ContractMd::from_markdown(&text);

    // Check header
    if contract.id.is_empty() {
        diags.push(
            Diagnostic::error("CTR-002", "Missing contract ID in header").with_file(file_label),
        );
    }
    if contract.version.is_empty() {
        diags.push(Diagnostic::error("CTR-003", "Missing version in header").with_file(file_label));
    } else if contract.version != entry.version {
        diags.push(
            Diagnostic::error(
                "CTR-004",
                format!(
                    "MD version '{}' doesn't match project.yaml version '{}'",
                    contract.version, entry.version
                ),
            )
            .with_file(file_label),
        );
    }

    // Check required sections
    let present = contract.present_section_names();
    let present_lower: Vec<String> = present.iter().map(|s| s.to_lowercase()).collect();
    for &required in ContractMd::required_sections() {
        if !present_lower.contains(&required.to_lowercase()) {
            diags.push(
                Diagnostic::error("CTR-010", format!("Missing required section: {}", required))
                    .with_file(file_label),
            );
        }
    }

    // Check sources reference existing files
    for source in &contract.sources {
        // Extract path from markdown link [text](path)
        if let Some(rel_path) = extract_link_path(source) {
            let contract_dir = path.parent().unwrap_or(root);
            let abs = contract_dir.join(rel_path);
            if !abs.exists() {
                diags.push(
                    Diagnostic::warning("CTR-020", format!("Source file not found: {}", rel_path))
                        .with_file(file_label),
                );
            }
        }
    }

    // Check Input/Output YAML parseable
    if let Some(ref yaml) = contract.input_yaml {
        if serde_yaml::from_str::<serde_yaml::Value>(yaml).is_err() {
            diags.push(
                Diagnostic::error("CTR-030", "Input YAML block is invalid").with_file(file_label),
            );
        }
    } else {
        diags.push(Diagnostic::error("CTR-031", "No Input YAML block found").with_file(file_label));
    }

    if let Some(ref yaml) = contract.output_yaml {
        if serde_yaml::from_str::<serde_yaml::Value>(yaml).is_err() {
            diags.push(
                Diagnostic::error("CTR-032", "Output YAML block is invalid").with_file(file_label),
            );
        }
    } else {
        diags
            .push(Diagnostic::error("CTR-033", "No Output YAML block found").with_file(file_label));
    }

    // Check glossary type references in YAML blocks
    if let Some(ref yaml) = contract.input_yaml {
        check_glossary_refs(yaml, glossary, file_label, &mut diags);
    }
    if let Some(ref yaml) = contract.output_yaml {
        check_glossary_refs(yaml, glossary, file_label, &mut diags);
    }

    // Check examples: at least 1 happy + 1 error
    if !contract.has_happy_path_example() {
        diags.push(
            Diagnostic::warning("CTR-040", "No happy-path example found").with_file(file_label),
        );
    }
    if !contract.has_error_example() {
        diags.push(Diagnostic::warning("CTR-041", "No error example found").with_file(file_label));
    }

    // Check errors table has entries
    if contract.errors.is_empty() {
        diags.push(
            Diagnostic::warning("CTR-050", "No errors defined in Errors table")
                .with_file(file_label),
        );
    }

    // Check invariants
    if contract.invariants.is_empty() {
        diags.push(Diagnostic::warning("CTR-051", "No invariants defined").with_file(file_label));
    }

    diags
}

fn check_contract_yaml(root: &Path, entry: &ContractEntry, yaml_path: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let path = root.join(yaml_path);
    let file_label = yaml_path;

    let contract = match ContractYaml::load(&path) {
        Ok(c) => c,
        Err(e) => {
            diags.push(
                Diagnostic::error("CTR-Y01", format!("Cannot parse YAML contract: {}", e))
                    .with_file(file_label),
            );
            return diags;
        }
    };

    if contract.id.is_empty() {
        diags.push(Diagnostic::error("CTR-Y02", "Missing contract id").with_file(file_label));
    }
    if contract.version.is_empty() {
        diags.push(Diagnostic::error("CTR-Y03", "Missing contract version").with_file(file_label));
    }

    // Check id matches entry
    if contract.id != entry.id {
        diags.push(
            Diagnostic::error(
                "CTR-Y10",
                format!(
                    "YAML id '{}' doesn't match project entry '{}'",
                    contract.id, entry.id
                ),
            )
            .with_file(file_label),
        );
    }

    // Check version matches entry
    if !contract.version.is_empty() && contract.version != entry.version {
        diags.push(
            Diagnostic::error(
                "CTR-Y11",
                format!(
                    "YAML version '{}' doesn't match project.yaml version '{}'",
                    contract.version, entry.version
                ),
            )
            .with_file(file_label),
        );
    }

    // Required fields
    if contract.inputs_schema.is_none() {
        diags.push(Diagnostic::error("CTR-Y20", "Missing inputs_schema").with_file(file_label));
    }
    if contract.outputs_schema.is_none() {
        diags.push(Diagnostic::error("CTR-Y21", "Missing outputs_schema").with_file(file_label));
    }
    if contract.errors.is_empty() {
        diags.push(Diagnostic::warning("CTR-Y22", "No errors defined").with_file(file_label));
    }
    if contract.invariants.is_empty() {
        diags.push(Diagnostic::warning("CTR-Y23", "No invariants defined").with_file(file_label));
    }

    diags
}

fn extract_link_path(md_text: &str) -> Option<&str> {
    // Try [text](path) format
    if let Some(start) = md_text.find("](") {
        let rest = &md_text[start + 2..];
        if let Some(end) = rest.find(')') {
            return Some(&rest[..end]);
        }
    }
    // Try __LINK_DEST:path__ format from our parser
    if let Some(start) = md_text.find("__LINK_DEST:") {
        let rest = &md_text[start + 12..];
        if let Some(end) = rest.find("__") {
            return Some(&rest[..end]);
        }
    }
    None
}

fn check_glossary_refs(
    yaml: &str,
    glossary: &Glossary,
    file_label: &str,
    diags: &mut Vec<Diagnostic>,
) {
    // Look for $ref: "glossary#TypeName" or $ref: "glossary.yaml#/types/TypeName"
    for line in yaml.lines() {
        if !line.contains("$ref") {
            continue;
        }
        if let Some(type_name) = extract_glossary_type_ref(line) {
            let known = glossary.all_type_names();
            if !known.contains(&type_name.as_str()) {
                diags.push(
                    Diagnostic::warning(
                        "CTR-060",
                        format!("Glossary type '{}' not found", type_name),
                    )
                    .with_file(file_label),
                );
            }
        }
    }
}

fn extract_glossary_type_ref(line: &str) -> Option<String> {
    // Match "glossary#TypeName" or "glossary.yaml#/types/TypeName"
    if let Some(pos) = line.find("glossary") {
        let rest = &line[pos..];
        if let Some(hash) = rest.find('#') {
            let after = &rest[hash + 1..];
            let type_part = after
                .trim_start_matches('/')
                .trim_start_matches("types/")
                .trim_end_matches('"')
                .trim_end_matches('\'')
                .trim();
            if !type_part.is_empty() {
                return Some(type_part.to_string());
            }
        }
    }
    None
}
