use crate::model::stage::StagePlan;
use crate::tui::app::App;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    if let Some(current) = app.milestones.as_ref().and_then(|m| m.current.clone()) {
        render_milestone(f, area, app, &current);
        return;
    }

    // No milestone — show empty state
    app.set_scroll_limit(0);
    f.render_widget(
        Paragraph::new("No active milestone. Run: hlv milestone new <name>")
            .block(Block::default().title(" Contracts ").borders(Borders::ALL)),
        area,
    );
}

fn render_milestone(
    f: &mut Frame,
    area: Rect,
    app: &mut App,
    current: &crate::model::milestone::MilestoneCurrent,
) {
    let header = Row::new(vec!["Contract", "Stage"])
        .style(Style::default().bold())
        .bottom_margin(1);

    // Scan milestone contracts dir for .md files
    let contracts_dir = app
        .project_root
        .join("human/milestones")
        .join(&current.id)
        .join("contracts");

    // Load stage plans to map contracts → stages
    let milestone_dir = app.project_root.join("human/milestones").join(&current.id);
    let stage_plans: Vec<StagePlan> = current
        .stages
        .iter()
        .filter_map(|s| {
            let path = milestone_dir.join(format!("stage_{}.md", s.id));
            StagePlan::load(&path).ok()
        })
        .collect();

    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if let Ok(entries) = std::fs::read_dir(&contracts_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(contract_id) = name.strip_suffix(".md") {
                if seen.insert(contract_id.to_string()) {
                    let stage_info = stage_plans
                        .iter()
                        .find(|sp| sp.contracts.iter().any(|c| c == contract_id))
                        .map(|sp| format!("Stage {}", sp.id))
                        .unwrap_or_else(|| "—".to_string());

                    rows.push(
                        Row::new(vec![contract_id.to_string(), stage_info])
                            .style(Style::default().fg(Color::Cyan)),
                    );
                }
            }
        }
    }

    let row_count = rows.len();
    let widths = [Constraint::Length(30), Constraint::Min(20)];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(format!(" Contracts ({}) ", current.id))
                .borders(Borders::ALL),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    app.set_scroll_limit(row_count.saturating_sub(1));
    let selected = (row_count > 0).then_some(app.selected_index.min(row_count - 1));
    let mut state = TableState::default().with_selected(selected);
    f.render_stateful_widget(table, area, &mut state);
}
