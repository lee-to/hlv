use crate::model::milestone::StageStatus;
use crate::model::policy::GatesPolicy;
use crate::model::task::TaskStatus;
use crate::tui::app::App;
use crate::tui::widgets::{status_color, vertical_scroll_offset};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(10)])
        .split(area);

    if let Some(current) = app.milestones.as_ref().and_then(|m| m.current.clone()) {
        render_milestone(f, chunks, app, &current);
        return;
    }

    // No milestone — show minimal info
    let project = match app.project.as_ref() {
        Some(p) => p,
        None => {
            app.set_scroll_limit(0);
            f.render_widget(Paragraph::new("No project loaded"), chunks[0]);
            return;
        }
    };

    let info = vec![
        Line::from(vec![
            Span::styled("Project: ", Style::default().bold()),
            Span::raw(project.project.clone()),
        ]),
        Line::from(Span::styled(
            "No active milestone. Run: hlv milestone new <name>",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    app.set_scroll_limit(0);
    f.render_widget(
        Paragraph::new(info)
            .block(Block::default().title(" Project ").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );
}

fn render_gates_summary(lines: &mut Vec<Line<'static>>, policy: &GatesPolicy) {
    lines.push(Line::from(Span::styled("Gates:", Style::default().bold())));
    let enabled = policy.gates.iter().filter(|g| g.enabled).count();
    let mandatory = policy.gates.iter().filter(|g| g.mandatory).count();
    let with_cmd = policy.gates.iter().filter(|g| g.command.is_some()).count();
    lines.push(Line::from(vec![Span::raw(format!(
        "  {}/{} enabled, {} mandatory, {} with command",
        enabled,
        policy.gates.len(),
        mandatory,
        with_cmd,
    ))]));
    // Show last gate results from milestone if available
    for gate in &policy.gates {
        let icon = if gate.enabled { "●" } else { "○" };
        let color = if gate.enabled {
            Color::Green
        } else {
            Color::DarkGray
        };
        let mandatory_flag = if gate.mandatory { " [M]" } else { "" };
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(icon, Style::default().fg(color)),
            Span::raw(" "),
            Span::raw(gate.id.clone()),
            Span::styled(
                mandatory_flag.to_string(),
                Style::default().fg(Color::Yellow),
            ),
        ]));
    }
}

fn render_milestone(
    f: &mut Frame,
    chunks: std::rc::Rc<[Rect]>,
    app: &mut App,
    current: &crate::model::milestone::MilestoneCurrent,
) {
    let project_name = app
        .project
        .as_ref()
        .map(|p| p.project.clone())
        .unwrap_or_else(|| "?".into());

    let validated = current
        .stages
        .iter()
        .filter(|s| s.status == StageStatus::Validated)
        .count();
    let total = current.stages.len();
    let progress = if total > 0 {
        format!("{}/{} stages validated", validated, total)
    } else {
        "no stages".to_string()
    };

    let info = vec![
        Line::from(vec![
            Span::styled("Project:   ", Style::default().bold()),
            Span::raw(project_name),
        ]),
        Line::from(vec![
            Span::styled("Milestone: ", Style::default().bold()),
            Span::styled(current.id.clone(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Progress:  ", Style::default().bold()),
            Span::raw(progress),
        ]),
    ];

    f.render_widget(
        Paragraph::new(info)
            .block(Block::default().title(" Milestone ").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    // Stages overview
    let mut lines: Vec<Line<'static>> =
        vec![Line::from(Span::styled("Stages:", Style::default().bold()))];

    for s in &current.stages {
        let icon = match s.status {
            StageStatus::Validated => "✓",
            StageStatus::Verified => "✓",
            StageStatus::Implementing | StageStatus::Validating => "▸",
            StageStatus::Implemented => "●",
            StageStatus::Pending => "○",
        };
        let status_str = s.status.to_string();
        let active = if current.stage == Some(s.id) {
            " ◀"
        } else {
            ""
        };
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(icon, Style::default().fg(status_color(&status_str))),
            Span::raw(format!(" Stage {}: ", s.id)),
            Span::styled(s.scope.clone(), Style::default()),
            Span::raw(" ["),
            Span::styled(
                status_str.clone(),
                Style::default().fg(status_color(&status_str)),
            ),
            Span::raw("]"),
            Span::styled(active.to_string(), Style::default().fg(Color::Cyan)),
        ]));

        if !s.tasks.is_empty() {
            let done = s
                .tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Done)
                .count();
            let in_progress = s
                .tasks
                .iter()
                .filter(|t| t.status == TaskStatus::InProgress)
                .count();
            let blocked = s
                .tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Blocked)
                .count();
            let total = s.tasks.len();
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    format!("Tasks: {}/{} done", done, total),
                    Style::default().fg(Color::DarkGray),
                ),
                if in_progress > 0 {
                    Span::styled(
                        format!(", {} active", in_progress),
                        Style::default().fg(Color::Yellow),
                    )
                } else {
                    Span::raw("")
                },
                if blocked > 0 {
                    Span::styled(
                        format!(", {} blocked", blocked),
                        Style::default().fg(Color::Red),
                    )
                } else {
                    Span::raw("")
                },
            ]));
        }
    }

    if current.stages.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No stages yet. Run /generate to create them.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // History summary
    let history = app
        .milestones
        .as_ref()
        .map(|m| m.history.clone())
        .unwrap_or_default();
    {
        if !history.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "History:",
                Style::default().bold(),
            )));
            for h in history.iter().rev().take(5) {
                let status_str = h.status.to_string();
                let date = h.merged_at.as_deref().unwrap_or("—");
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("{:03}", h.number),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(" "),
                    Span::raw(h.id.clone()),
                    Span::raw(" — "),
                    Span::styled(
                        status_str.clone(),
                        Style::default().fg(status_color(&status_str)),
                    ),
                    Span::raw(" "),
                    Span::styled(date.to_string(), Style::default().fg(Color::DarkGray)),
                ]));
            }
        }
    }

    // Git info
    if let Some(ref project) = app.project {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled("Git:", Style::default().bold())));
        if let Some(ref branch) = current.branch {
            lines.push(Line::from(vec![
                Span::raw("  Branch:     "),
                Span::styled(branch.clone(), Style::default().fg(Color::Cyan)),
            ]));
        }
        lines.push(Line::from(vec![
            Span::raw("  Convention: "),
            Span::raw(project.git.commit_convention.to_string()),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  Merge:      "),
            Span::raw(project.git.merge_strategy.to_string()),
        ]));
    }

    // Gates summary
    if let Some(ref policy) = app.gates_policy {
        lines.push(Line::raw(""));
        render_gates_summary(&mut lines, policy);
    }

    let visible_rows = chunks[1].height.saturating_sub(2) as usize;
    let scroll_limit = lines.len().saturating_sub(visible_rows);
    app.set_scroll_limit(scroll_limit);

    f.render_widget(
        Paragraph::new(lines)
            .block(Block::default().title(" Overview ").borders(Borders::ALL))
            .scroll((vertical_scroll_offset(app.selected_index), 0))
            .wrap(Wrap { trim: true }),
        chunks[1],
    );
}
