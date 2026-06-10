use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub fn pad_display_width(s: &str, width: usize) -> String {
    let current = display_width(s);
    if current >= width {
        return s.to_string();
    }
    format!("{}{}", s, " ".repeat(width - current))
}

pub fn truncate_display_width(s: &str, max_width: usize) -> String {
    if display_width(s) <= max_width {
        return s.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    if max_width == 1 {
        return "…".to_string();
    }

    let mut out = String::new();
    let mut used = 0usize;
    for ch in s.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width + 1 > max_width {
            break;
        }
        out.push(ch);
        used += ch_width;
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_width_handles_wide_and_zero_width_chars() {
        assert_eq!(display_width("abc"), 3);
        assert_eq!(display_width("語"), 2);
        assert_eq!(display_width("e\u{301}"), 1);
    }

    #[test]
    fn padding_uses_terminal_width() {
        assert_eq!(display_width(&pad_display_width("語", 4)), 4);
    }

    #[test]
    fn truncation_includes_ellipsis_within_width() {
        let text = truncate_display_width("ab語cd", 5);
        assert_eq!(text, "ab語…");
        assert_eq!(display_width(&text), 5);
    }

    #[test]
    fn padded_box_rows_align_by_display_width() {
        let row_a = format!("│{}│", pad_display_width("ASCII", 8));
        let row_b = format!("│{}│", pad_display_width("語e\u{301}", 8));

        assert_eq!(display_width(&row_a), 10);
        assert_eq!(display_width(&row_b), 10);
    }
}
