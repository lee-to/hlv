use crate::tui::app::{App, InputMode};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    // Build title with milestone context if available
    let title = if let Some(ref ms) = app.milestones {
        if let Some(ref current) = ms.current {
            let stage_info = current
                .stage
                .and_then(|sid| current.stages.iter().find(|s| s.id == sid))
                .map(|s| format!(" | stage {} ({})", s.id, s.status))
                .unwrap_or_default();
            format!(" Gates — {}{} ", current.id, stage_info)
        } else {
            " Gates ".to_string()
        }
    } else {
        " Gates ".to_string()
    };

    // If editing a gate command or cwd, split area for input
    let is_editing = app.input_mode == InputMode::Answering
        && (app.editing_gate_command.is_some() || app.editing_gate_cwd.is_some());
    let chunks = if is_editing {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0)])
            .split(area)
    };

    let header = Row::new(vec![
        "Gate",
        "Type",
        "Mandatory",
        "Enabled",
        "Cwd",
        "Command",
    ])
    .style(Style::default().bold())
    .bottom_margin(1);

    let (rows, row_count) = {
        let policy = match app.gates_policy.as_ref() {
            Some(p) => p,
            None => {
                app.set_scroll_limit(0);
                return;
            }
        };

        let rows: Vec<Row> = policy
            .gates
            .iter()
            .map(|g| {
                let style = if g.enabled {
                    Style::default()
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                Row::new(vec![
                    g.id.clone(),
                    g.gate_type.clone(),
                    if g.mandatory {
                        "yes".into()
                    } else {
                        "no".into()
                    },
                    if g.enabled {
                        "yes".into()
                    } else {
                        "off".into()
                    },
                    g.cwd.clone().unwrap_or_else(|| ".".into()),
                    g.command.clone().unwrap_or_else(|| "—".into()),
                ])
                .style(style)
            })
            .collect();

        (rows, policy.gates.len())
    };

    let widths = [
        Constraint::Length(25),
        Constraint::Length(22),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Min(20),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().title(title).borders(Borders::ALL))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    app.set_scroll_limit(row_count.saturating_sub(1));
    let selected = (row_count > 0).then_some(app.selected_index.min(row_count - 1));
    let mut state = TableState::default().with_selected(selected);
    f.render_stateful_widget(table, chunks[0], &mut state);

    // Render input field if editing command or cwd
    if is_editing {
        let editing_idx = app
            .editing_gate_command
            .or(app.editing_gate_cwd)
            .unwrap_or(0);
        let gate_name = app
            .gates_policy
            .as_ref()
            .and_then(|p| p.gates.get(editing_idx))
            .map(|g| g.id.as_str())
            .unwrap_or("?");
        let field_name = if app.editing_gate_cwd.is_some() {
            "Cwd"
        } else {
            "Command"
        };

        let input = Paragraph::new(app.input_buffer.as_str()).block(
            Block::default()
                .title(format!(
                    " {} for {} (Enter to save, Esc to cancel) ",
                    field_name, gate_name
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(input, chunks[1]);

        // Position cursor
        let x = chunks[1].x + app.input_buffer.len() as u16 + 1;
        let y = chunks[1].y + 1;
        f.set_cursor_position((x.min(chunks[1].right() - 2), y));
    }
}
