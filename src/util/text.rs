use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn truncate_ellipsis(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    if s.chars().count() <= max_chars {
        return s.to_string();
    }

    let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

pub fn truncate_display_ellipsis(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    if UnicodeWidthStr::width(s) <= max_width {
        return s.to_string();
    }

    if max_width == 1 {
        return "…".to_string();
    }

    let content_width = max_width.saturating_sub(1);
    let mut out = String::new();
    let mut width = 0;

    for ch in s.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > content_width {
            break;
        }
        width += ch_width;
        out.push(ch);
    }

    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_ellipsis_returns_empty_for_zero_limit() {
        assert_eq!(truncate_ellipsis("abc", 0), "");
    }

    #[test]
    fn truncate_ellipsis_keeps_exact_char_limit() {
        assert_eq!(truncate_ellipsis("abc", 3), "abc");
    }

    #[test]
    fn truncate_ellipsis_truncates_ascii_with_single_ellipsis_char() {
        assert_eq!(truncate_ellipsis("abcdef", 4), "abc…");
    }

    #[test]
    fn truncate_ellipsis_does_not_panic_on_multibyte_boundary() {
        let input = format!("{}Жtail", "a".repeat(199));

        let truncated = truncate_ellipsis(&input, 200);

        assert_eq!(truncated.chars().count(), 200);
        assert!(truncated.ends_with('…'));
        assert_eq!(truncated, format!("{}…", "a".repeat(199)));
    }

    #[test]
    fn truncate_display_ellipsis_returns_empty_for_zero_width() {
        assert_eq!(truncate_display_ellipsis("abc", 0), "");
    }

    #[test]
    fn truncate_display_ellipsis_returns_ellipsis_for_one_width() {
        assert_eq!(truncate_display_ellipsis("abc", 1), "…");
    }

    #[test]
    fn truncate_display_ellipsis_truncates_ascii_by_display_width() {
        assert_eq!(truncate_display_ellipsis("abcdef", 4), "abc…");
    }

    #[test]
    fn truncate_display_ellipsis_respects_cjk_display_width() {
        assert_eq!(truncate_display_ellipsis("界界a", 4), "界…");
    }

    #[test]
    fn truncate_display_ellipsis_respects_emoji_display_width() {
        assert_eq!(truncate_display_ellipsis("😀😀a", 4), "😀…");
    }
}
