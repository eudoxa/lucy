use ansi_to_tui::IntoText;
use once_cell::sync::Lazy;
use ratatui::text::{Line, Span};
use regex::Regex;

use crate::theme::{ANSI_RESET, ColorExt, THEME};

static RE_STARTED: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"Started (?P<method>[A-Z]+) "(?P<path>[^"]+)""#).unwrap());
static RE_PROCESSING: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"Processing by (?P<controller>[\w:]+)#(?P<action>\w+) as (?P<format>\w+)"#)
        .unwrap()
});
static RE_PARAMETERS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"Parameters: \{(?P<params>.*)\}"#).unwrap());
static RE_SQL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(SELECT|INSERT|UPDATE|DELETE).*"#).unwrap());
static RE_COMPLETED: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"Completed (?P<status>[0-9]+) [\w\s]+ in (?P<time>[0-9]+)ms"#).unwrap()
});
static RE_CONTINUATION: Lazy<Regex> = Lazy::new(|| Regex::new(r#"↳"#).unwrap());

pub fn format_simple_log_line(line: &str) -> Option<Line<'static>> {
    let core_message = if let Some(index) = line.rfind("] ") {
        line.split_at(index + 2).1
    } else {
        line
    };

    if let Some(captures) = RE_COMPLETED.captures(core_message) {
        let status = captures.name("status").unwrap().as_str();
        let colored_message = match status.chars().next().unwrap() {
            '2' => format!("{}{}{}", THEME.success.ansi(), core_message, ANSI_RESET), // green
            '4' => format!("{}{}{}", THEME.warning.ansi(), core_message, ANSI_RESET), // yellow
            '5' => format!("{}{}{}", THEME.error.ansi(), core_message, ANSI_RESET),   // red
            _ => core_message.to_string(),
        };
        Some(Line::from(parse_ansi_colors(&colored_message)))
    } else if RE_STARTED.is_match(core_message)
        || RE_PROCESSING.is_match(core_message)
        || RE_PARAMETERS.is_match(core_message)
        || (RE_SQL.is_match(core_message) && !core_message.contains("CACHE"))
        || RE_CONTINUATION.is_match(core_message)
    {
        Some(Line::from(parse_ansi_colors(core_message)))
    } else {
        None
    }
}

pub fn parse_ansi_colors(text: &str) -> Vec<Span<'static>> {
    match text.into_text() {
        Ok(parsed_text) => {
            if !parsed_text.lines.is_empty() {
                parsed_text.lines[0].spans.clone()
            } else {
                vec![Span::raw(text.to_string())]
            }
        }
        Err(_) => {
            vec![Span::raw(text.to_string())]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ansi_colors() {
        let plain_text = "Hello, world!";
        let spans = parse_ansi_colors(plain_text);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "Hello, world!");

        let colored_text = "\\x1b[31mRed text\\x1b[0m";
        let spans = parse_ansi_colors(colored_text);
        assert!(!spans.is_empty());
        assert!(spans.iter().any(|span| span.content.contains("Red text")));
    }

    // Add tests for format_simple_log_line if needed
}
