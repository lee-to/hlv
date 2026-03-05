use crate::tui::app::App;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    if let Some(ref ms) = app.milestones {
        if let Some(ref current) = ms.current {
            let oq_path = app
                .project_root
                .join("human/milestones")
                .join(&current.id)
                .join("open-questions.md");
            if oq_path.exists() {
                render_milestone_questions(f, area, app, &oq_path);
                return;
            }
        }
    }

    // No milestone or no open-questions.md yet
    app.set_scroll_limit(0);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "No open questions (run /generate to create them)",
            Style::default().fg(Color::DarkGray),
        )))
        .block(
            Block::default()
                .title(" Open Questions ")
                .borders(Borders::ALL),
        ),
        area,
    );
}

/// Render open questions from milestone open-questions.md (read-only view).
fn render_milestone_questions(f: &mut Frame, area: Rect, app: &mut App, oq_path: &std::path::Path) {
    let content = match std::fs::read_to_string(oq_path) {
        Ok(c) => c,
        Err(_) => {
            app.set_scroll_limit(0);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "Cannot read open-questions.md",
                    Style::default().fg(Color::Red),
                )))
                .block(
                    Block::default()
                        .title(" Open Questions (milestone) ")
                        .borders(Borders::ALL),
                ),
                area,
            );
            return;
        }
    };

    let question_lines: Vec<&str> = content
        .lines()
        .filter(|l| {
            let t = l.trim();
            t.starts_with("- [ ]") || t.starts_with("- [x]") || t.starts_with("- [deferred]")
        })
        .collect();

    if question_lines.is_empty() {
        app.set_scroll_limit(0);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No open questions",
                Style::default().fg(Color::DarkGray),
            )))
            .block(
                Block::default()
                    .title(" Open Questions (milestone) ")
                    .borders(Borders::ALL),
            ),
            area,
        );
        return;
    }

    let selected = app.selected_index;
    let mut lines: Vec<Line> = Vec::new();
    let mut question_start_lines: Vec<usize> = Vec::new();

    for (i, raw_line) in question_lines.iter().enumerate() {
        question_start_lines.push(lines.len());
        let trimmed = raw_line.trim();

        let (marker, style, rest) = if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            ("[ ]", Style::default().fg(Color::Yellow), rest)
        } else if let Some(rest) = trimmed.strip_prefix("- [x] ") {
            ("[x]", Style::default().fg(Color::Green), rest)
        } else if let Some(rest) = trimmed.strip_prefix("- [deferred] ") {
            ("[~]", Style::default().fg(Color::DarkGray), rest)
        } else {
            ("[?]", Style::default().fg(Color::DarkGray), trimmed)
        };

        let is_selected = i == selected;
        let row_style = if is_selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };

        lines.push(Line::from(vec![
            Span::styled(format!("{marker} "), style),
            Span::styled(rest.to_string(), row_style),
        ]));
        lines.push(Line::raw(""));
    }

    let scroll_limit = question_lines.len().saturating_sub(1);
    let scroll_target = question_start_lines.get(selected).copied().unwrap_or(0);

    app.set_scroll_limit(scroll_limit);

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Open Questions (milestone) ")
                    .borders(Borders::ALL),
            )
            .scroll((scroll_target as u16, 0))
            .wrap(Wrap { trim: true }),
        area,
    );
}
