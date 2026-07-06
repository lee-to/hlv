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
            code: "PRJ-090",
            title: "Legacy mode missing code roots",
            meaning: "features.legacy_mode is enabled, but paths.code.src is missing or empty.",
            common_causes: &["adopted project.yaml was hand-written without paths.code", "source roots were not selected during adoption"],
            fixes: &["add paths.code.src entries for observed source roots such as app/, internal/, src/", "disable features.legacy_mode if this is a greenfield project"],
        },
        DiagnosticExplanation {
            code: "PRJ-091",
            title: "Legacy source root missing",
            meaning: "A paths.code.src directory does not exist at the repository root.",
            common_causes: &["source root typo", "running hlv check with the wrong --root", "project layout changed after adoption"],
            fixes: &["fix the paths.code.src entry", "create the missing source directory", "run hlv check --root from the repository root"],
        },
        DiagnosticExplanation {
            code: "PRJ-092",
            title: "Legacy test root missing",
            meaning: "A paths.code.tests directory does not exist at the repository root.",
            common_causes: &["test root typo", "tests live beside source files", "project layout changed after adoption"],
            fixes: &["fix or remove the paths.code.tests entry", "create the missing test directory", "use the source root as a test root for languages that colocate tests"],
        },
        DiagnosticExplanation {
            code: "PRJ-093",
            title: "paths.code without legacy mode",
            meaning: "paths.code is configured but features.legacy_mode is false, so HLV treats the project as greenfield and ignores observed code roots.",
            common_causes: &["legacy_mode was omitted", "copied an adopted project.yaml section into a greenfield project"],
            fixes: &["set features.legacy_mode: true for adopted projects", "or remove paths.code from greenfield projects"],
        },
        DiagnosticExplanation {
            code: "CTR-060",
            title: "Glossary reference not found",
            meaning: "A contract references a glossary type or enum, but the target was not found in human/glossary.yaml.",
            common_causes: &["typo in $ref", "missing type in glossary", "wrong section: types vs enums"],
            fixes: &["add the type or enum to glossary.yaml", "or update the contract $ref"],
        },
        DiagnosticExplanation {
            code: "CTR-010",
            title: "Missing contract section or code trace marker",
            meaning: "A contract is missing a required Markdown section, or a contract/error/invariant/constraint ID has no matching @hlv marker in source or tests.",
            common_causes: &["contract Markdown does not include all required sections", "implementation code was added without @hlv markers", "a contract or constraint ID was renamed without updating markers"],
            fixes: &["add the missing contract section", "add or update the @hlv marker near the implementation or test", "run hlv check again after renaming IDs"],
        },
        DiagnosticExplanation {
            code: "TST-020",
            title: "No contract tests found",
            meaning: "A test-spec file does not declare any CT-* contract test IDs.",
            common_causes: &["the Contract Tests section is empty", "test IDs do not start with CT-", "test cases are described without declaring an ID"],
            fixes: &["add at least one CT-* test for happy paths and error cases", "declare IDs as headings, bullets, or Markdown table rows with the ID in the first cell", "map each test to a GATE-* reference"],
        },
        DiagnosticExplanation {
            code: "TST-021",
            title: "No property-based tests found",
            meaning: "A test-spec file does not declare any PBT-* property-based test IDs.",
            common_causes: &["the Property-Based Tests section is empty", "invariants were not converted into PBT-* specs", "test IDs do not start with PBT-"],
            fixes: &["add a PBT-* test for each contract invariant", "declare IDs as headings, bullets, or Markdown table rows with the ID in the first cell", "include generator, assertion, and gate details"],
        },
        DiagnosticExplanation {
            code: "CST-050",
            title: "Constraint rule command failed",
            meaning: "A constraint rule check_command ran and returned a failure.",
            common_causes: &["the implementation violates the rule", "the command cannot start", "the command uses unsupported shell syntax"],
            fixes: &["fix the implementation", "fix the command or cwd", "split shell pipelines into portable commands"],
        },
        DiagnosticExplanation {
            code: "GAT-050",
            title: "Gate command failed",
            meaning: "An enabled gate command ran during hlv check and returned a failure.",
            common_causes: &["the implementation failed the gate", "the command cannot start", "the command uses unsupported shell syntax"],
            fixes: &["fix the failing implementation or test", "fix the gate command or cwd", "use validation.strictness: relaxed only when gate execution should be skipped"],
        },
        DiagnosticExplanation {
            code: "TRC-022",
            title: "Mapping references unknown test ID",
            meaning: "A traceability mapping points at a test ID that HLV could not find in the referenced test-spec files.",
            common_causes: &["typo in traceability.yaml tests", "test was planned but not added to a test spec", "project.yaml contract entry points at the wrong test_spec file"],
            fixes: &["add the missing test ID to the relevant test spec", "fix the ID in traceability.yaml", "ensure the contract entry's test_spec path points to the file that declares the test"],
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
            code: "IDX-010",
            title: "Signature index stale",
            meaning: "The signature index does not match the current observed source files.",
            common_causes: &["source changed after the last index build", "the index file is missing after a fresh clone", "fingerprints were generated from a different checkout"],
            fixes: &["run hlv index build", "commit or regenerate .hlv/index/signatures.yaml according to the project index policy"],
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
