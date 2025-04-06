use crate::app::App;
use crate::layout::Panel;
use crate::log_parser::strip_ansi_for_parsing;
use crate::sql_info::QueryType;
use ansi_to_tui::IntoText;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph, Wrap},
};

const INDEX_OFFSET: usize = 1;
const UI_OVERHEAD: usize = 4;

pub fn build_list_component(app: &App) -> List<'_> {
    let mut items = Vec::with_capacity(app.state.request_ids.len());

    let viewport_height = app.app_view.viewport_height(Panel::RequestList);
    let current_offset = app.app_view.get_scroll_offset(Panel::RequestList);
    let visible_count =
        viewport_height.min(app.state.request_ids.len().saturating_sub(current_offset));
    let end_idx = current_offset + visible_count;

    for index in current_offset..end_idx {
        if index >= app.state.request_ids.len() {
            break;
        }

        let request_id = &app.state.request_ids[index];
        let time_str = app.state.first_timestamps.get(request_id).map_or_else(
            || "??:??:??".to_string(),
            |ts| ts.format("%H:%M:%S").to_string(),
        );

        let group = app.state.logs_by_request_id.get(request_id).unwrap();
        let finished = group.finished;
        let title = group.title.clone();

        let log_count = group.entries.len();
        let sql_count = group.sql_query_info.total_queries();

        let content = Line::from(vec![
            Span::raw(format!("{} ", time_str)),
            Span::styled(
                format!("{:2}-{:2} ", log_count, sql_count),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                title,
                Style::default().fg(if finished { Color::Green } else { Color::White }),
            ),
        ]);

        let style = if index == app.state.selected_index {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if finished {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::White)
        };

        items.push(ListItem::new(content).style(style));
    }

    let border_style = match app.app_view.focused_panel {
        Panel::RequestList => Style::default().fg(Color::White),
        _ => Style::default().fg(Color::DarkGray),
    };

    List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .padding(Padding::new(1, 1, 1, 1))
            .title(" Requests"),
    )
}

pub fn build_detail_component(app: &App) -> Paragraph<'_> {
    let (_title_span, log_text) = match app.state.selected_group() {
        None => (Span::raw("Logs"), Text::from("Waiting for logs...")),
        Some(group) => {
            let title_span = if group.finished {
                Span::styled(
                    "Completed",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(
                    "Running",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            };

            let mut text = Text::default();
            if let Some(entry) = group.entries.iter().find(|entry| {
                let msg = &entry.message;
                msg.contains("Started GET")
                    || msg.contains("Started POST")
                    || msg.contains("Started PUT")
                    || msg.contains("Started DELETE")
            }) {
                let styled_line = parse_ansi_colors(&entry.message);
                let mut spans = styled_line;
                for span in &mut spans {
                    span.style = span.style.add_modifier(Modifier::BOLD);
                }
                text.extend(Text::from(Line::from(spans)));
                text.extend(Text::from(Line::from("")));
            }

            let viewport_height = app.app_view.viewport_height(Panel::RequestDetail);
            let total_entries = group.entries.len();
            let mut visible_logs = Vec::with_capacity(viewport_height.min(total_entries));

            let detail_scroll_offset = app.app_view.get_scroll_offset(Panel::RequestDetail);
            let start_idx = detail_scroll_offset.min(total_entries.saturating_sub(1));
            let visible_count = viewport_height.min(total_entries.saturating_sub(start_idx));

            for i in 0..visible_count {
                let idx = total_entries - 1 - (start_idx + i);
                if idx < total_entries {
                    visible_logs.push(&group.entries[idx]);
                }
            }

            for log in visible_logs {
                let timestamp = log.timestamp.format("%H:%M:%S%.3f").to_string();
                let message = if let Some(after_id) = strip_ansi_for_parsing(&log.message).find(']')
                {
                    let raw_message = &log.message[(after_id + 1)..].trim();
                    raw_message.to_string()
                } else {
                    log.message.clone()
                };
                let mut spans = vec![Span::styled(
                    format!("[{}] ", timestamp),
                    Style::default().fg(Color::Gray),
                )];
                spans.extend(parse_ansi_colors(&message));
                text.extend(Text::from(Line::from(spans)));
            }

            (title_span, text)
        }
    };

    let border_style = match app.app_view.focused_panel {
        Panel::RequestDetail => Style::default().fg(Color::White),
        _ => Style::default().fg(Color::DarkGray),
    };
    let paragraph = Paragraph::new(log_text);

    let scroll_info = if let Some(group) = app.state.selected_group() {
        let total_entries = group.entries.len();
        if total_entries == 0 {
            "0/0".to_string()
        } else {
            let detail_scroll_offset = app.app_view.get_scroll_offset(Panel::RequestDetail);
            let start_idx = detail_scroll_offset + INDEX_OFFSET;
            let viewport_height = app.app_view.viewport_height(Panel::RequestDetail);
            let need_height =
                paragraph.line_count(app.app_view.get_viewport_width(Panel::RequestDetail) as u16);
            let overflow_lines = need_height.saturating_sub(viewport_height);
            let end_idx =
                (start_idx + viewport_height - UI_OVERHEAD - overflow_lines).min(total_entries);
            format!("{}-{}/{}", start_idx, end_idx, total_entries)
        }
    } else {
        "0/0".to_string()
    };

    let title_text = format!(" [{}] ", scroll_info);
    let title_style = match app.app_view.focused_panel {
        Panel::RequestDetail => Style::default().fg(Color::Yellow),
        _ => Style::default().fg(Color::White),
    };
    let block = Block::default()
        .padding(Padding::new(1, 1, 1, 1))
        .title_alignment(ratatui::layout::Alignment::Right)
        .title(Span::styled(title_text, title_style))
        .borders(Borders::ALL)
        .border_style(border_style);

    paragraph.block(block).wrap(Wrap { trim: true })
}

pub fn build_log_stream_component(app: &App) -> Paragraph<'_> {
    let mut log_text = Text::default();
    let total_logs = app.state.all_logs.len();

    let viewport_height = app.app_view.viewport_height(Panel::LogStream);

    if total_logs > 0 {
        let visible_logs = app.get_visible_logs(viewport_height);

        for log in visible_logs.into_iter() {
            let timestamp = log.timestamp.format("%H:%M:%S%.3f").to_string();

            let mut spans = vec![Span::styled(
                format!("[{}] ", timestamp),
                Style::default().fg(Color::Gray),
            )];

            spans.extend(parse_ansi_colors(&log.message));
            log_text.extend(Text::from(Line::from(spans)));
        }
    }

    let copy_mode_text = if app.copy_mode_enabled {
        " COPY MODE (press 'm' to exit) "
    } else {
        " j/k: scroll | Ctrl+d/u: page | Tab/Shift+Tab: panels | Ctrl+c: quit | m: copy mode | f: toggle adaptive FPS "
    };

    let scroll_info = if total_logs == 0 {
        "0/0".to_string()
    } else {
        let all_scroll_offset = app.app_view.get_scroll_offset(Panel::LogStream);
        let start_idx = all_scroll_offset + INDEX_OFFSET;
        let end_idx = (start_idx + viewport_height - INDEX_OFFSET).min(total_logs);
        format!("{}-{}/{}", start_idx, end_idx, total_logs)
    };

    let border_style = match app.app_view.focused_panel {
        Panel::LogStream => Style::default().fg(Color::White),
        _ => Style::default().fg(Color::DarkGray),
    };
    let bottom_style = match app.app_view.focused_panel {
        Panel::LogStream => Style::default().fg(Color::Yellow),
        _ => Style::default().fg(Color::DarkGray),
    };

    let title_text = format!(" All Logs Stream [{}] ", scroll_info);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .padding(Padding::new(1, 1, 1, 1))
        .title(title_text)
        .title_alignment(ratatui::layout::Alignment::Left)
        .title_bottom(
            Line::from(vec![Span::styled(copy_mode_text, bottom_style)])
                .alignment(ratatui::layout::Alignment::Right),
        );

    Paragraph::new(log_text)
        .block(block)
        .wrap(Wrap { trim: true })
        .alignment(ratatui::layout::Alignment::Left)
}

pub fn build_sql_component(app: &App) -> Paragraph<'_> {
    let border_style = match app.app_view.focused_panel {
        Panel::SqlInfo => Style::default().fg(Color::White),
        _ => Style::default().fg(Color::DarkGray),
    };

    let mut text = Text::default();
    if let Some(group) = app.state.selected_group() {
        let sql_info = &group.sql_query_info;

        text.extend(Text::from(Line::from("")));

        text.extend(Text::from(Line::from(vec![
            Span::styled("SELECT: ", Style::default().fg(Color::Green)),
            Span::raw(sql_info.query_count(QueryType::Select).to_string()),
        ])));

        text.extend(Text::from(Line::from(vec![
            Span::styled("INSERT: ", Style::default().fg(Color::Yellow)),
            Span::raw(sql_info.query_count(QueryType::Insert).to_string()),
        ])));

        text.extend(Text::from(Line::from(vec![
            Span::styled("UPDATE: ", Style::default().fg(Color::Magenta)),
            Span::raw(sql_info.query_count(QueryType::Update).to_string()),
        ])));

        text.extend(Text::from(Line::from(vec![
            Span::styled("DELETE: ", Style::default().fg(Color::Red)),
            Span::raw(sql_info.query_count(QueryType::Delete).to_string()),
        ])));

        if !sql_info.table_counts.is_empty() {
            text.extend(Text::from(Line::from("")));
            for (table, count) in sql_info.sorted_tables() {
                text.extend(Text::from(Line::from(vec![
                    Span::styled(
                        format!("{}: ", table),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(count.to_string()),
                ])));
            }
        }
    }

    let scroll_info = if let Some(group) = app.state.selected_group() {
        let total_queries = group.sql_query_info.total_queries();
        if total_queries == 0 {
            "0/0".to_string()
        } else {
            format!("{}", total_queries)
        }
    } else {
        "0/0".to_string()
    };

    let title_text = format!(" SQL [{}] ", scroll_info);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .padding(Padding::new(1, 1, 0, 0))
        .title(title_text);

    let sql_scroll_offset = app.app_view.get_scroll_offset(Panel::SqlInfo);

    Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: true })
        .scroll((sql_scroll_offset as u16, 0))
}

fn parse_ansi_colors(text: &str) -> Vec<Span<'static>> {
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

        let colored_text = "\x1b[31mRed text\x1b[0m";
        let spans = parse_ansi_colors(colored_text);
        assert!(spans.len() >= 1);
        assert!(spans.iter().any(|span| span.content.contains("Red text")));
    }
}
