#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommand {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandParseError {
    EmptyCommand,
    MissingProgram,
    UnmatchedQuote,
    UnsupportedSyntax(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteMode {
    Single,
    Double,
}

pub fn parse_portable_command(
    command: &str,
) -> std::result::Result<ParsedCommand, CommandParseError> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(CommandParseError::EmptyCommand);
    }

    if let Some(operator) = find_unsupported_shell_syntax(trimmed) {
        return Err(CommandParseError::UnsupportedSyntax(operator));
    }

    let mut parts = split_command_line(trimmed)?.into_iter();
    let program = parts.next().ok_or(CommandParseError::MissingProgram)?;
    let args = parts.collect();

    Ok(ParsedCommand { program, args })
}

fn split_command_line(command: &str) -> std::result::Result<Vec<String>, CommandParseError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut quote_mode: Option<QuoteMode> = None;

    while let Some(ch) = chars.next() {
        match quote_mode {
            Some(QuoteMode::Single) => {
                if ch == '\'' {
                    quote_mode = None;
                } else {
                    current.push(ch);
                }
            }
            Some(QuoteMode::Double) => {
                if ch == '"' {
                    quote_mode = None;
                } else if ch == '\\' {
                    if let Some(next) = chars.peek().copied() {
                        if next == '"' {
                            let mut lookahead = chars.clone();
                            let _ = lookahead.next();
                            if should_close_after_backslash_quote(lookahead.clone()) {
                                current.push(ch);
                                quote_mode = None;
                            } else if has_unescaped_double_quote(lookahead) {
                                current.push(next);
                            } else {
                                current.push(ch);
                                quote_mode = None;
                            }
                            let _ = chars.next();
                        } else {
                            current.push(ch);
                        }
                    } else {
                        current.push(ch);
                    }
                } else {
                    current.push(ch);
                }
            }
            None => {
                if ch.is_whitespace() {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                } else if ch == '\'' {
                    quote_mode = Some(QuoteMode::Single);
                } else if ch == '"' {
                    quote_mode = Some(QuoteMode::Double);
                } else {
                    current.push(ch);
                }
            }
        }
    }

    if quote_mode.is_some() {
        return Err(CommandParseError::UnmatchedQuote);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    if tokens.is_empty() {
        return Err(CommandParseError::EmptyCommand);
    }

    Ok(tokens)
}

fn should_close_after_backslash_quote(mut chars: std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    match chars.peek().copied() {
        None => true,
        Some(ch) if is_shell_delimiter(ch) => true,
        Some(ch) if ch.is_whitespace() => {
            while let Some(next) = chars.peek().copied() {
                if next.is_whitespace() {
                    let _ = chars.next();
                } else {
                    break;
                }
            }
            match chars.peek().copied() {
                None => true,
                Some('"') => true,
                Some(ch) if is_shell_delimiter(ch) => true,
                _ => false,
            }
        }
        _ => false,
    }
}

fn has_unescaped_double_quote(mut chars: std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    while let Some(ch) = chars.next() {
        if ch == '"' {
            return true;
        }
        if ch == '\\' && chars.peek().copied() == Some('"') {
            let _ = chars.next();
        }
    }
    false
}

fn is_shell_delimiter(ch: char) -> bool {
    matches!(ch, '&' | '|' | ';' | '<' | '>')
}

fn find_unsupported_shell_syntax(command: &str) -> Option<&'static str> {
    let mut chars = command.chars().peekable();
    let mut quote_mode: Option<QuoteMode> = None;

    while let Some(ch) = chars.next() {
        match quote_mode {
            Some(QuoteMode::Single) => {
                if ch == '\'' {
                    quote_mode = None;
                }
            }
            Some(QuoteMode::Double) => {
                if ch == '"' {
                    quote_mode = None;
                } else if ch == '\\' && chars.peek().copied() == Some('"') {
                    let _ = chars.next();
                }
            }
            None => match ch {
                '\'' => quote_mode = Some(QuoteMode::Single),
                '"' => quote_mode = Some(QuoteMode::Double),
                '&' => {
                    if chars.peek().copied() == Some('&') {
                        return Some("&&");
                    }
                    return Some("&");
                }
                '|' => {
                    if chars.peek().copied() == Some('|') {
                        return Some("||");
                    }
                    return Some("|");
                }
                ';' => return Some(";"),
                '>' => return Some(">"),
                '<' => return Some("<"),
                '`' => return Some("`"),
                '$' => {
                    if chars.peek().copied() == Some('(') {
                        return Some("$(");
                    }
                    if chars.peek().copied() == Some('{') {
                        return Some("${");
                    }
                    if let Some(next) = chars.peek().copied() {
                        if next.is_ascii_alphabetic() || next == '_' {
                            return Some("$VAR");
                        }
                    }
                }
                _ => {}
            },
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_supports_quoted_arguments() {
        let parsed = parse_portable_command(r#"cargo run --message "hello world""#).unwrap();
        assert_eq!(parsed.program, "cargo");
        assert_eq!(
            parsed.args,
            vec![
                "run".to_string(),
                "--message".to_string(),
                "hello world".to_string()
            ]
        );
    }

    #[test]
    fn parse_handles_windows_path_in_quotes() {
        let parsed = parse_portable_command(r#""C:\Program Files\tool.exe" --help"#).unwrap();
        assert_eq!(parsed.program, r"C:\Program Files\tool.exe");
        assert_eq!(parsed.args, vec!["--help".to_string()]);
    }

    #[test]
    fn parse_preserves_unc_prefix_in_quotes() {
        let parsed = parse_portable_command(r#""\\server\share\tool.exe" --help"#).unwrap();
        assert_eq!(parsed.program, r"\\server\share\tool.exe");
        assert_eq!(parsed.args, vec!["--help".to_string()]);
    }

    #[test]
    fn parse_preserves_trailing_backslash_before_closing_quote() {
        let parsed = parse_portable_command("cmd /C exit /B 0 \"C:\\tmp\\\"").unwrap();
        assert_eq!(parsed.program, "cmd");
        assert_eq!(
            parsed.args,
            vec![
                "/C".to_string(),
                "exit".to_string(),
                "/B".to_string(),
                "0".to_string(),
                r"C:\tmp\".to_string()
            ]
        );
    }

    #[test]
    fn parse_handles_trailing_backslash_before_quote_with_next_quoted_arg() {
        let parsed = parse_portable_command("cmd /C echo \"C:\\tmp\\\" \"next arg\"").unwrap();
        assert_eq!(
            parsed.args,
            vec![
                "/C".to_string(),
                "echo".to_string(),
                r"C:\tmp\".to_string(),
                "next arg".to_string()
            ]
        );
    }

    #[test]
    fn parse_rejects_unmatched_quote() {
        let err = parse_portable_command(r#"cargo --message "broken"#).unwrap_err();
        assert_eq!(err, CommandParseError::UnmatchedQuote);
    }

    #[test]
    fn parse_rejects_shell_operators() {
        let err = parse_portable_command("cargo test && cargo clippy").unwrap_err();
        assert_eq!(err, CommandParseError::UnsupportedSyntax("&&"));
    }

    #[test]
    fn parse_ignores_operator_chars_inside_quotes() {
        let parsed = parse_portable_command(r#"echo "a && b""#).unwrap();
        assert_eq!(parsed.program, "echo");
        assert_eq!(parsed.args, vec!["a && b".to_string()]);
    }

    #[test]
    fn parse_rejects_shell_variable_syntax() {
        let err = parse_portable_command("echo $HOME").unwrap_err();
        assert_eq!(err, CommandParseError::UnsupportedSyntax("$VAR"));
    }
}
