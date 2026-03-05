use anyhow::Result;
use colored::Colorize;
use std::path::Path;

use super::style;
use crate::model::glossary::Glossary;
use crate::model::project::ProjectMap;

/// Returns glossary as data (no stdout). Returns None if no glossary file exists.
pub fn get_glossary(root: &Path) -> Result<Option<Glossary>> {
    let project = ProjectMap::load(&root.join("project.yaml"))?;
    let glossary_path = root.join(&project.paths.human.glossary);

    if !glossary_path.exists() {
        return Ok(None);
    }

    Ok(Some(Glossary::load(&glossary_path)?))
}

pub fn run(root: &Path, json: bool) -> Result<()> {
    let project = ProjectMap::load(&root.join("project.yaml"))?;
    let glossary_path = root.join(&project.paths.human.glossary);

    if !glossary_path.exists() {
        if json {
            println!("null");
        } else {
            style::hint("No glossary found.");
        }
        return Ok(());
    }

    let glossary = Glossary::load(&glossary_path)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&glossary)?);
        return Ok(());
    }

    style::header("glossary");

    if let Some(ref domain) = glossary.domain {
        style::detail("Domain", domain);
    }

    if !glossary.types.is_empty() {
        style::section("Types");
        for (name, t) in &glossary.types {
            let extra = t.format.as_deref().unwrap_or("");
            println!(
                "    {} {} ({}{})",
                "·".dimmed(),
                name.bold(),
                t.kind,
                if extra.is_empty() {
                    String::new()
                } else {
                    format!(", {}", extra)
                }
            );
        }
    }

    if !glossary.enums.is_empty() {
        style::section("Enums");
        for (name, values) in &glossary.enums {
            println!(
                "    {} {} = [{}]",
                "·".dimmed(),
                name.bold(),
                values.join(", ")
            );
        }
    }

    if !glossary.terms.is_empty() {
        style::section("Terms");
        for (name, term) in &glossary.terms {
            println!("    {} {} — {}", "·".dimmed(), name.bold(), term.definition);
        }
    }

    if !glossary.rules.is_empty() {
        style::section("Rules");
        for rule in &glossary.rules {
            println!(
                "    {} {}: {}",
                "·".dimmed(),
                rule.id.bold(),
                rule.description
            );
        }
    }

    println!();
    Ok(())
}
