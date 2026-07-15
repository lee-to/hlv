use std::path::Path;

use regex::Regex;

use crate::check::Diagnostic;
use crate::model::policy::TraceabilityPolicy;

pub const TRACEABILITY_POLICY_PATH: &str = "validation/traceability-policy.yaml";

/// Load the optional project-specific test ID pattern.
///
/// Built-in HLV prefixes remain supported by the Markdown parser. The policy
/// pattern extends that set so projects can introduce their own test ID
/// convention without changing the HLV binary.
pub fn load_additional_test_id_pattern(root: &Path) -> (Option<Regex>, Vec<Diagnostic>) {
    let policy_path = root.join(TRACEABILITY_POLICY_PATH);
    if !policy_path.exists() {
        return (None, Vec::new());
    }

    let policy = match TraceabilityPolicy::load(&policy_path) {
        Ok(policy) => policy,
        Err(error) => {
            return (
                None,
                vec![Diagnostic::error(
                    "TRC-002",
                    format!("Cannot parse traceability policy: {error}"),
                )
                .with_file(TRACEABILITY_POLICY_PATH)],
            );
        }
    };

    let Some(pattern) = policy.id_formats.and_then(|formats| formats.test) else {
        return (None, Vec::new());
    };

    match Regex::new(&pattern) {
        Ok(regex) => (Some(regex), Vec::new()),
        Err(error) => (
            None,
            vec![
                Diagnostic::error("TRC-003", format!("Invalid id_formats.test regex: {error}"))
                    .with_file(TRACEABILITY_POLICY_PATH),
            ],
        ),
    }
}
