use crate::model::policy::{ConstraintFile, PerformanceConstraints};
use crate::model::project::ConstraintEntry;
use crate::tui::app::App;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let constraints: Vec<ConstraintEntry> = match app.project.as_ref() {
        Some(p) if !p.constraints.is_empty() => p.constraints.clone(),
        Some(_) => {
            let msg = Paragraph::new(
                "  No constraints defined.\n  Use `hlv constraints add <name>` to add one.",
            )
            .block(
                Block::default()
                    .title(" Constraints ")
                    .borders(Borders::ALL),
            );
            f.render_widget(msg, area);
            return;
        }
        None => {
            let msg = Paragraph::new("No project loaded").block(
                Block::default()
                    .title(" Constraints ")
                    .borders(Borders::ALL),
            );
            f.render_widget(msg, area);
            return;
        }
    };

    let root = app.project_root.clone();
    let mut lines: Vec<Line> = Vec::new();
    let mut total_rules = 0u32;
    let mut by_severity = [0u32; 4]; // critical, high, medium, low

    for entry in &constraints {
        let file_path = root.join(&entry.path);

        // Try rule-based first, then performance
        if let Ok(cf) = ConstraintFile::load(&file_path) {
            lines.push(render_constraint_header(entry, false));
            for rule in &cf.rules {
                total_rules += 1;
                match rule.severity.as_str() {
                    "critical" => by_severity[0] += 1,
                    "high" => by_severity[1] += 1,
                    "medium" => by_severity[2] += 1,
                    "low" => by_severity[3] += 1,
                    _ => {}
                }
                let sev_style = severity_style(&rule.severity);
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(rule.id.clone(), Style::default().fg(Color::White)),
                    Span::raw("  "),
                    Span::styled(rule.severity.clone(), sev_style),
                    Span::raw("  "),
                    Span::styled(
                        truncate(&rule.statement, 60),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        } else if let Ok(perf) = PerformanceConstraints::load(&file_path) {
            lines.push(render_constraint_header(entry, true));
            if let Some(ref defaults) = perf.defaults {
                let mut parts = Vec::new();
                if let Some(v) = defaults.latency_p95_ms {
                    parts.push(format!("p95={v}ms"));
                }
                if let Some(v) = defaults.latency_p99_ms {
                    parts.push(format!("p99={v}ms"));
                }
                if let Some(v) = defaults.error_rate_max_percent {
                    parts.push(format!("error_rate<{v}%"));
                }
                lines.push(Line::from(vec![
                    Span::raw("    defaults: "),
                    Span::styled(parts.join(", "), Style::default().fg(Color::DarkGray)),
                ]));
            }
            if !perf.overrides.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("    overrides: {}", perf.overrides.len()),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled("  ! ", Style::default().fg(Color::Red)),
                Span::raw(&entry.id),
                Span::styled("  (cannot load)", Style::default().fg(Color::Red)),
            ]));
        }
        lines.push(Line::raw(""));
    }

    // Summary
    let summary = format!(
        "  {} constraints, {} rules ({} critical, {} high, {} medium, {} low)",
        constraints.len(),
        total_rules,
        by_severity[0],
        by_severity[1],
        by_severity[2],
        by_severity[3],
    );
    lines.push(Line::from(Span::styled(
        summary,
        Style::default().fg(Color::DarkGray),
    )));

    let scroll_limit = lines.len().saturating_sub(1);
    let scroll_offset = app.selected_index;
    app.set_scroll_limit(scroll_limit);

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Constraints ")
                .borders(Borders::ALL),
        )
        .scroll((scroll_offset as u16, 0));

    f.render_widget(paragraph, area);
}

fn render_constraint_header(entry: &ConstraintEntry, metric_based: bool) -> Line<'static> {
    let suffix = if metric_based { "  (metric-based)" } else { "" };
    Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(entry.id.clone(), Style::default().fg(Color::Cyan).bold()),
        Span::raw("  "),
        Span::styled(entry.path.clone(), Style::default().fg(Color::DarkGray)),
        Span::styled(suffix.to_string(), Style::default().fg(Color::Yellow)),
    ])
}

fn severity_style(severity: &str) -> Style {
    match severity {
        "critical" => Style::default().fg(Color::Red).bold(),
        "high" => Style::default().fg(Color::Yellow),
        "medium" => Style::default().fg(Color::Blue),
        "low" => Style::default().fg(Color::DarkGray),
        _ => Style::default(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}
