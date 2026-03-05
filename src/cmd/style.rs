use colored::Colorize;
use std::cell::Cell;

const WIDTH: usize = 60;

thread_local! {
    static QUIET: Cell<bool> = const { Cell::new(false) };
}

/// Set quiet mode: when true, all style output is suppressed.
/// Used by MCP tools to prevent stdout pollution on the JSON-RPC channel.
pub fn set_quiet(quiet: bool) {
    QUIET.with(|q| q.set(quiet));
}

/// Check if quiet mode is active.
pub fn is_quiet() -> bool {
    QUIET.with(|q| q.get())
}

/// Print command header: "hlv <name>" + separator
pub fn header(name: &str) {
    if is_quiet() {
        return;
    }
    println!();
    println!("  {}{}", "hlv ".bold(), name.bold());
    separator();
}

/// Print a thin separator line
pub fn separator() {
    if is_quiet() {
        return;
    }
    println!("  {}", "─".repeat(WIDTH).dimmed());
}

/// Print section heading: "▶ Title"
pub fn section(title: &str) {
    if is_quiet() {
        return;
    }
    println!("\n  {} {}", "▶".blue(), title.bold());
}

/// Print success: "✓ message"
pub fn ok(msg: &str) {
    if is_quiet() {
        return;
    }
    println!("  {} {}", "✓".green(), msg);
}

/// Print fatal error: "✗ message" (for unrecoverable errors before exit)
/// Note: uses stderr, so NOT suppressed by quiet mode.
pub fn fatal(msg: &str) {
    eprintln!("\n  {} {}", "✗".red().bold(), msg);
}

/// Print warning line
pub fn warn(msg: &str) {
    if is_quiet() {
        return;
    }
    println!("  {} {}", "!".yellow().bold(), msg);
}

/// Print info/hint line
pub fn hint(msg: &str) {
    if is_quiet() {
        return;
    }
    println!("  {}", msg.dimmed());
}

/// Print a detail line (indented)
pub fn detail(label: &str, value: &str) {
    if is_quiet() {
        return;
    }
    println!("    {}: {}", label.dimmed(), value);
}

/// Print a file operation line (create/update/skip/mkdir)
pub fn file_op(verb: &str, path: &str, note: Option<&str>) {
    if is_quiet() {
        return;
    }
    let colored_verb = match verb {
        "create" => verb.green(),
        "update" => verb.cyan(),
        "skip" => verb.yellow(),
        "delete" => verb.red(),
        "mkdir" => verb.dimmed(),
        _ => verb.normal(),
    };
    match note {
        Some(n) => println!("    {} {} ({})", colored_verb, path, n.dimmed()),
        None => println!("    {} {}", colored_verb, path),
    }
}

/// Format an anyhow error chain into a readable string
pub fn format_error(err: &anyhow::Error) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(err.to_string());
    let mut source = err.source();
    while let Some(cause) = source {
        parts.push(cause.to_string());
        source = std::error::Error::source(cause);
    }
    parts.join("\n    caused by: ")
}
