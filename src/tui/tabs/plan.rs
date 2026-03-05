use crate::cmd::plan::resolve_task_status;
use crate::model::milestone::StageStatus;
use crate::model::stage::StagePlan;
use crate::model::task::TaskStatus;
use crate::tui::app::App;
use crate::tui::widgets::{status_color, vertical_scroll_offset};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    if let Some(current) = app.milestones.as_ref().and_then(|m| m.current.clone()) {
        render_milestone(f, area, app, &current);
        return;
    }

    // No milestone — show empty state
    app.set_scroll_limit(0);
    f.render_widget(
        Paragraph::new("No active milestone. Run: hlv milestone new <name>")
            .block(Block::default().title(" Plan ").borders(Borders::ALL)),
        area,
    );
}

fn render_milestone(
    f: &mut Frame,
    area: Rect,
    app: &mut App,
    current: &crate::model::milestone::MilestoneCurrent,
) {
    let mut lines = Vec::new();

    if current.stages.is_empty() {
        lines.push(Line::from(Span::styled(
            "No stages yet. Run /generate to create them.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for stage_entry in &current.stages {
            let status_str = stage_entry.status.to_string();
            let icon = match stage_entry.status {
                StageStatus::Validated => "✓",
                StageStatus::Verified => "✓",
                StageStatus::Implementing | StageStatus::Validating => "▸",
                StageStatus::Implemented => "●",
                StageStatus::Pending => "○",
            };
            let active = if current.stage == Some(stage_entry.id) {
                " ◀"
            } else {
                ""
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} Stage {}: ", icon, stage_entry.id),
                    Style::default().fg(status_color(&status_str)),
                ),
                Span::styled(stage_entry.scope.clone(), Style::default().bold()),
                Span::raw(" ["),
                Span::styled(
                    status_str,
                    Style::default().fg(status_color(&stage_entry.status.to_string())),
                ),
                Span::raw("]"),
                Span::styled(active.to_string(), Style::default().fg(Color::Cyan)),
            ]));

            // Try to load stage tasks
            let stage_file = app
                .project_root
                .join("human/milestones")
                .join(&current.id)
                .join(format!("stage_{}.md", stage_entry.id));
            if let Ok(stage) = StagePlan::load(&stage_file) {
                for task in &stage.tasks {
                    let status = resolve_task_status(task, stage_entry);
                    let (task_icon, task_color) = match status.as_ref() {
                        Some(TaskStatus::Done) => ("✓", Color::Green),
                        Some(TaskStatus::InProgress) => ("▸", Color::Yellow),
                        Some(TaskStatus::Blocked) => ("✗", Color::Red),
                        _ => ("○", Color::DarkGray),
                    };
                    let deps = if task.depends_on.is_empty() {
                        String::new()
                    } else {
                        format!(" (after {})", task.depends_on.join(", "))
                    };
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("    {} ", task_icon),
                            Style::default().fg(task_color),
                        ),
                        Span::styled(task.id.clone(), Style::default()),
                        Span::raw(format!(" {}", task.name)),
                        Span::styled(deps, Style::default().fg(Color::DarkGray)),
                    ]));
                }
                if !stage.remediation.is_empty() {
                    for fix in &stage.remediation {
                        lines.push(Line::from(vec![
                            Span::styled("    ! ", Style::default().fg(Color::Red)),
                            Span::styled(fix.id.clone(), Style::default().fg(Color::Red)),
                            Span::raw(format!(" {}", fix.name)),
                        ]));
                    }
                }
            }
            lines.push(Line::raw(""));
        }
    }

    let visible_rows = area.height.saturating_sub(2) as usize;
    let scroll_limit = lines.len().saturating_sub(visible_rows);
    app.set_scroll_limit(scroll_limit);

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Plan (Stages) ")
                    .borders(Borders::ALL),
            )
            .scroll((vertical_scroll_offset(app.selected_index), 0))
            .wrap(Wrap { trim: true }),
        area,
    );
}
