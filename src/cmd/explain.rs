use anyhow::Result;
use colored::Colorize;

use super::style;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiagnosticExplanation {
    pub code: &'static str,
    pub title: &'static str,
    pub meaning: &'static str,
    pub common_causes: &'static [&'static str],
    pub fixes: &'static [&'static str],
}

pub fn lookup_diagnostic(code: &str) -> Option<&'static DiagnosticExplanation> {
    let wanted = code.to_ascii_uppercase();
    registry().iter().find(|entry| entry.code == wanted)
}

pub fn suggest_diagnostics(code: &str) -> Vec<&'static DiagnosticExplanation> {
    let wanted = code.to_ascii_uppercase();
    let prefix = wanted
        .split_once('-')
        .map(|(prefix, _)| prefix)
        .unwrap_or("");
    registry()
        .iter()
        .filter(|entry| entry.code.starts_with(prefix))
        .take(8)
        .collect()
}

pub fn run(code: &str) -> Result<()> {
    match lookup_diagnostic(code) {
        Some(entry) => print_explanation(entry),
        None => {
            style::fatal(&format!("Unknown diagnostic code: {}", code));
            let suggestions = suggest_diagnostics(code);
            if !suggestions.is_empty() {
                eprintln!();
                eprintln!("  Did you mean:");
                for suggestion in suggestions {
                    eprintln!("    {}: {}", suggestion.code, suggestion.title);
                }
            }
        }
    }
    Ok(())
}

fn print_explanation(entry: &DiagnosticExplanation) {
    println!();
    println!("  {}: {}", entry.code.bold(), entry.title.bold());
    println!();
    println!("  {}", "Meaning:".bold());
    println!("    {}", entry.meaning);
    println!();
    println!("  {}", "Common causes:".bold());
    for cause in entry.common_causes {
        println!("    - {}", cause);
    }
    println!();
    println!("  {}", "Fix:".bold());
    for fix in entry.fixes {
        println!("    - {}", fix);
    }
}

fn registry() -> &'static [DiagnosticExplanation] {
    &[
        DiagnosticExplanation {
            code: "PRJ-001",
            title: "Cannot parse project.yaml",
            meaning: "HLV could not load the project map, so later checks cannot run reliably.",
            common_causes: &["invalid YAML", "unknown project.yaml fields", "wrong schema version"],
            fixes: &["fix the YAML syntax", "remove unknown fields", "run hlv doctor for path and schema checks"],
        },
        DiagnosticExplanation {
            code: "PRJ-030",
            title: "Glossary type missing",
            meaning: "project.yaml lists a glossary type that is not defined in human/glossary.yaml.",
            common_causes: &["typo in glossary_types", "type deleted from glossary", "glossary not regenerated after artifact changes"],
            fixes: &["add the type to human/glossary.yaml", "or remove/update the glossary_types entry"],
        },
        DiagnosticExplanation {
            code: "CTR-060",
            title: "Glossary reference not found",
            meaning: "A contract references a glossary type or enum, but the target was not found in human/glossary.yaml.",
            common_causes: &["typo in $ref", "missing type in glossary", "wrong section: types vs enums"],
            fixes: &["add the type or enum to glossary.yaml", "or update the contract $ref"],
        },
        DiagnosticExplanation {
            code: "CST-050",
            title: "Constraint rule command failed",
            meaning: "A constraint rule check_command ran and returned a failure.",
            common_causes: &["the implementation violates the rule", "the command cannot start", "the command uses unsupported shell syntax"],
            fixes: &["fix the implementation", "fix the command or cwd", "split shell pipelines into portable commands"],
        },
        DiagnosticExplanation {
            code: "MAP-080",
            title: "Generated source outside paths.llm.src",
            meaning: "Generated implementation ownership points outside the configured LLM source directory.",
            common_causes: &["map.yaml entry uses layer: llm for app source", "artifact_graph code-* path points to a non-LLM directory", "paths.llm.src is configured incorrectly"],
            fixes: &["move generated source under paths.llm.src", "or update paths.llm.src and map.yaml consistently"],
        },
        DiagnosticExplanation {
            code: "MAP-081",
            title: "Generated tests outside paths.llm.tests",
            meaning: "Generated test ownership points outside the configured LLM tests directory.",
            common_causes: &["tests-* ownership path points to repository tests", "paths.llm.tests is missing or incorrect"],
            fixes: &["move generated tests under paths.llm.tests", "or update paths.llm.tests and ownership metadata"],
        },
        DiagnosticExplanation {
            code: "WVR-020",
            title: "Expired waiver",
            meaning: "A waiver reached its expiry date and no longer suppresses diagnostics.",
            common_causes: &["legacy cleanup was not completed before expiry", "expiry date was set too aggressively"],
            fixes: &["fix the underlying diagnostic", "or renew the waiver with a new reason and expiry"],
        },
        DiagnosticExplanation {
            code: "DOC-001",
            title: "project.yaml not found",
            meaning: "Doctor could not find project.yaml at the inspected root.",
            common_causes: &["running outside an HLV project", "wrong --root value"],
            fixes: &["run from the project root", "or pass --root to the intended HLV project"],
        },
    ]
}
