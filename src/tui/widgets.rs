use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Gauge};

/// Render a progress gauge.
pub fn progress_gauge<'a>(title: &'a str, ratio: f64, color: Color) -> Gauge<'a> {
    let pct = (ratio * 100.0) as u16;
    Gauge::default()
        .block(Block::default().title(title).borders(Borders::ALL))
        .gauge_style(Style::default().fg(color))
        .ratio(ratio.clamp(0.0, 1.0))
        .label(format!("{}%", pct))
}

/// Color for a status string.
pub fn status_color(status: &str) -> Color {
    match status {
        "passed" | "verified" | "completed" | "implemented" | "validated" => Color::Green,
        "failed" => Color::Red,
        "in_progress" | "implementing" | "validating" => Color::Yellow,
        _ => Color::DarkGray,
    }
}

/// Status symbol.
pub fn status_symbol(status: &str) -> &'static str {
    match status {
        "passed" | "verified" | "completed" | "implemented" => "■",
        "failed" => "✗",
        "in_progress" | "implementing" => "▶",
        _ => "○",
    }
}

/// Convert the shared selection index into a safe paragraph scroll offset.
pub fn vertical_scroll_offset(selected_index: usize) -> u16 {
    selected_index.min(u16::MAX as usize) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_color_green_variants() {
        for s in &[
            "passed",
            "verified",
            "completed",
            "implemented",
            "validated",
        ] {
            assert_eq!(status_color(s), Color::Green, "expected Green for '{}'", s);
        }
    }

    #[test]
    fn status_color_red() {
        assert_eq!(status_color("failed"), Color::Red);
    }

    #[test]
    fn status_color_yellow_variants() {
        for s in &["in_progress", "implementing", "validating"] {
            assert_eq!(
                status_color(s),
                Color::Yellow,
                "expected Yellow for '{}'",
                s
            );
        }
    }

    #[test]
    fn status_color_default() {
        assert_eq!(status_color("unknown"), Color::DarkGray);
    }

    #[test]
    fn status_symbol_variants() {
        assert_eq!(status_symbol("passed"), "■");
        assert_eq!(status_symbol("verified"), "■");
        assert_eq!(status_symbol("completed"), "■");
        assert_eq!(status_symbol("implemented"), "■");
        assert_eq!(status_symbol("failed"), "✗");
        assert_eq!(status_symbol("in_progress"), "▶");
        assert_eq!(status_symbol("implementing"), "▶");
    }

    #[test]
    fn status_symbol_default() {
        assert_eq!(status_symbol("unknown"), "○");
    }

    #[test]
    fn vertical_scroll_offset_saturates() {
        assert_eq!(vertical_scroll_offset(7), 7);
        assert_eq!(vertical_scroll_offset(usize::MAX), u16::MAX);
    }
}
