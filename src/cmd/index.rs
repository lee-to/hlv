use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;

use super::style;
use crate::model::index::{Index, Symbol};

pub fn run_build(project_root: &Path) -> Result<()> {
    let summary = crate::index::builder::build_index(project_root)?;
    style::header("index build");
    println!(
        "  {} scanned {} file(s), indexed {} symbol(s)",
        "✓".green().bold(),
        summary.files_scanned,
        summary.symbols_indexed
    );
    style::detail("output", &summary.output.display().to_string());
    Ok(())
}

pub fn run_show(project_root: &Path, symbol: &str, json: bool) -> Result<()> {
    let matches = find_symbols(project_root, symbol)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&matches)?);
        return Ok(());
    }

    style::header("index show");
    if matches.is_empty() {
        style::warn(&format!("No symbol found for {symbol}"));
        return Ok(());
    }
    for symbol in matches {
        print_symbol(&symbol);
    }
    Ok(())
}

pub fn run_list(project_root: &Path, file: &str, json: bool) -> Result<()> {
    let symbols = list_symbols_by_file(project_root, file)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&symbols)?);
        return Ok(());
    }

    style::header("index list");
    style::detail("file", file);
    if symbols.is_empty() {
        style::warn("No symbols found");
        return Ok(());
    }
    for symbol in symbols {
        print_symbol(&symbol);
    }
    Ok(())
}

pub fn find_symbols(project_root: &Path, selector: &str) -> Result<Vec<Symbol>> {
    let index = load_index(project_root)?;
    tracing::debug!(selector = selector, "Querying signature index");
    let matches = index
        .symbols
        .into_iter()
        .filter(|symbol| symbol.id == selector || symbol.name == selector)
        .collect::<Vec<_>>();
    tracing::debug!(
        selector = selector,
        result_count = matches.len(),
        "Signature index query complete"
    );
    Ok(matches)
}

pub fn list_symbols_by_file(project_root: &Path, file: &str) -> Result<Vec<Symbol>> {
    let index = load_index(project_root)?;
    tracing::debug!(file = file, "Listing signature index file symbols");
    let symbols = index
        .symbols
        .into_iter()
        .filter(|symbol| symbol.file == file)
        .collect::<Vec<_>>();
    tracing::debug!(
        file = file,
        result_count = symbols.len(),
        "Signature index file listing complete"
    );
    Ok(symbols)
}

fn load_index(project_root: &Path) -> Result<Index> {
    let path = crate::config_root(project_root).join("index/signatures.yaml");
    Index::load(&path).with_context(|| format!("failed to load {}", path.display()))
}

fn print_symbol(symbol: &Symbol) {
    println!(
        "  {} {} {}:{}",
        "·".dimmed(),
        symbol.id.bold(),
        symbol.file.dimmed(),
        symbol.line
    );
    println!("    {}", symbol.signature);
}
