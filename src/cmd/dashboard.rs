use std::io;
use std::path::Path;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Tabs};

use crate::tui::app::{App, InputMode, Tab};
use crate::tui::tabs;

pub fn run(project_root: &Path) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(project_root);

    // Main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    while app.running {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            // Clear status message on any keypress
            app.status_message = None;

            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.quit(),
                    KeyCode::Tab | KeyCode::Right => app.next_tab(),
                    KeyCode::BackTab | KeyCode::Left => app.prev_tab(),
                    KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
                    KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
                    KeyCode::Char('r') => app.reload(),
                    // Gates tab actions
                    KeyCode::Char('e') if app.current_tab == Tab::Gates => {
                        app.toggle_gate();
                    }
                    KeyCode::Char('c') if app.current_tab == Tab::Gates => {
                        app.start_editing_gate_command();
                    }
                    KeyCode::Char('d') if app.current_tab == Tab::Gates => {
                        app.delete_gate();
                    }
                    KeyCode::Char('x') if app.current_tab == Tab::Gates => {
                        app.clear_gate_command();
                    }
                    KeyCode::Char('w') if app.current_tab == Tab::Gates => {
                        app.start_editing_gate_cwd();
                    }
                    _ => {}
                },
                InputMode::Answering => match key.code {
                    KeyCode::Enter => {
                        if app.editing_gate_command.is_some() {
                            app.submit_gate_command();
                        } else if app.editing_gate_cwd.is_some() {
                            app.submit_gate_cwd();
                        } else {
                            app.cancel_input();
                        }
                    }
                    KeyCode::Esc => app.cancel_input(),
                    KeyCode::Backspace => {
                        app.input_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        // Ctrl+U clears input
                        if c == 'u' && key.modifiers.contains(KeyModifiers::CONTROL) {
                            app.input_buffer.clear();
                        } else {
                            app.input_buffer.push(c);
                        }
                    }
                    _ => {}
                },
            }
        }
    }

    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    // Tab bar
    let tab_titles: Vec<Line> = Tab::all().iter().map(|t| Line::from(t.title())).collect();

    let tab_index = Tab::all()
        .iter()
        .position(|t| *t == app.current_tab)
        .unwrap_or(0);

    let tab_bar = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" HLV Dashboard "),
        )
        .select(tab_index)
        .style(Style::default())
        .highlight_style(Style::default().fg(Color::Cyan).bold());

    f.render_widget(tab_bar, chunks[0]);

    // Content
    match app.current_tab {
        Tab::Status => tabs::status::render(f, chunks[1], app),
        Tab::Contracts => tabs::contracts::render(f, chunks[1], app),
        Tab::Plan => tabs::plan::render(f, chunks[1], app),
        Tab::Gates => tabs::gates::render(f, chunks[1], app),
        Tab::Constraints => tabs::constraints::render(f, chunks[1], app),
        Tab::Questions => tabs::questions::render(f, chunks[1], app),
    }

    // Footer — context-sensitive
    let footer = match app.input_mode {
        InputMode::Answering => Line::from(vec![
            Span::styled(" Enter", Style::default().bold()),
            Span::raw(" submit  "),
            Span::styled("Esc", Style::default().bold()),
            Span::raw(" cancel  "),
            Span::styled("C-u", Style::default().bold()),
            Span::raw(" clear"),
        ]),
        InputMode::Normal if app.current_tab == Tab::Gates => Line::from(vec![
            Span::styled(" q", Style::default().bold()),
            Span::raw(" quit  "),
            Span::styled("Tab", Style::default().bold()),
            Span::raw(" switch  "),
            Span::styled("↑↓", Style::default().bold()),
            Span::raw(" scroll  "),
            Span::styled("e", Style::default().bold()),
            Span::raw(" on/off  "),
            Span::styled("c", Style::default().bold()),
            Span::raw(" command  "),
            Span::styled("w", Style::default().bold()),
            Span::raw(" cwd  "),
            Span::styled("d", Style::default().bold()),
            Span::raw(" delete  "),
            Span::styled("x", Style::default().bold()),
            Span::raw(" clear cmd  "),
            Span::styled("r", Style::default().bold()),
            Span::raw(" reload"),
        ]),
        InputMode::Normal if app.current_tab == Tab::Questions => Line::from(vec![
            Span::styled(" q", Style::default().bold()),
            Span::raw(" quit  "),
            Span::styled("Tab", Style::default().bold()),
            Span::raw(" switch  "),
            Span::styled("↑↓", Style::default().bold()),
            Span::raw(" scroll  "),
            Span::styled("a/Enter", Style::default().bold()),
            Span::raw(" answer  "),
            Span::styled("d", Style::default().bold()),
            Span::raw(" defer  "),
            Span::styled("o", Style::default().bold()),
            Span::raw(" reopen  "),
            Span::styled("r", Style::default().bold()),
            Span::raw(" reload"),
        ]),
        _ => Line::from(vec![
            Span::styled(" q", Style::default().bold()),
            Span::raw(" quit  "),
            Span::styled("Tab", Style::default().bold()),
            Span::raw(" switch  "),
            Span::styled("↑↓", Style::default().bold()),
            Span::raw(" scroll  "),
            Span::styled("r", Style::default().bold()),
            Span::raw(" reload"),
        ]),
    };

    let footer_with_status = if let Some(ref msg) = app.status_message {
        let mut spans = vec![
            Span::styled(format!(" {msg} "), Style::default().fg(Color::Green)),
            Span::raw("│ "),
        ];
        spans.extend(footer.spans);
        Line::from(spans)
    } else {
        footer
    };

    f.render_widget(
        ratatui::widgets::Paragraph::new(footer_with_status)
            .style(Style::default().fg(Color::DarkGray)),
        chunks[2],
    );
}
