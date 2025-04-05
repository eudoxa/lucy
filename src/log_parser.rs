use crate::app::LogEntry;
use chrono::Local;
use once_cell::sync::Lazy;
use regex::Regex;

static ANSI_ESCAPE_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\x1b\[[0-9;]*[mK]").expect("Invalid ANSI escape sequence regex"));

pub fn parse(line: &str) -> Option<LogEntry> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    let request_id = if trimmed.starts_with('[') {
        let cleaned = if line.contains("\x1b[") {
            strip_ansi_for_parsing(line)
        } else {
            line.to_string()
        };
        extract_request_id(&cleaned).unwrap_or_default()
    } else {
        String::new()
    };

    Some(LogEntry {
        request_id,
        timestamp: Local::now(),
        message: line.to_string(),
    })
}

// Temporarily remove ANSI escape sequences for parsing
pub fn strip_ansi_for_parsing(text: &str) -> String {
    if !text.contains("\x1b[") {
        return text.to_string();
    }
    ANSI_ESCAPE_PATTERN.replace_all(text, "").to_string()
}

fn extract_request_id(line: &str) -> Option<String> {
    if !line.starts_with('[') {
        return None;
    }

    let start = 0;
    let end = line[start..].find(']')?;
    let request_id = line[start + 1..end].trim();

    if !request_id.is_empty() {
        Some(request_id.to_string())
    } else {
        None
    }
}
