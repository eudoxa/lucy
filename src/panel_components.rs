use crate::app::App;
use crate::layout::Panel;
use crate::log_parser::strip_ansi_for_parsing;
use crate::simple_formatter::{format_simple_log_line, parse_ansi_colors};
use crate::sql_info::QueryType;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph, Wrap},
};

const INDEX_OFFSET: usize = 1;

pub fn build_list_component(app: &App) -> List<'_> {
    let mut items = Vec::with_capacity(app.state.log_group_count());

    let viewport_height = app.app_view.viewport_height(Panel::RequestList);
    let current_offset = app.app_view.get_scroll_offset(Panel::RequestList);
    let visible_count =
        viewport_height.min(app.state.log_group_count().saturating_sub(current_offset));
    let end_idx = current_offset + visible_count;

    for index in current_offset..end_idx {
        if index >= app.state.log_group_count() {
            break;
        }

        let request_id = app.state.request_ids()[index];
        let group = app.state.logs_by_request_id.get(request_id).unwrap();
        let time_str = group.first_timestamp.format("%H:%M:%S").to_string();

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

    let total_requests = app.state.log_group_count();
    let scroll_info = if total_requests == 0 {
        "0/0".to_string()
    } else {
        let start_idx = current_offset + INDEX_OFFSET;
        let end_idx = (start_idx + visible_count - INDEX_OFFSET).min(total_requests);
        format!("{}-{}/{}", start_idx, end_idx, total_requests)
    };

    let title_text = format!("[{}]", scroll_info);
    let title_style = match app.app_view.focused_panel {
        Panel::RequestList => Style::default().fg(Color::Yellow),
        _ => Style::default().fg(Color::White),
    };

    List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .padding(Padding::new(1, 1, 1, 1))
            .title(Span::styled(title_text, title_style)),
    )
}

pub fn build_detail_component(app: &App) -> Paragraph<'_> {
    let (title_span, log_text) = match app.state.selected_group() {
        None => (Span::raw("Logs"), Text::from("Waiting for logs...")),
        Some(group) => {
            let mut text = Text::default();
            let title_span = if let Some(entry) = group.entries.iter().find(|entry| {
                let msg = &entry.message;
                msg.contains("Started GET")
                    || msg.contains("Started POST")
                    || msg.contains("Started PUT")
                    || msg.contains("Started DELETE")
            }) {
                let msg = strip_ansi_for_parsing(&entry.message);

                // Extract HTTP method directly
                let method = msg
                    .split_whitespace()
                    .skip_while(|&s| s != "Started")
                    .nth(1)
                    .unwrap_or("");

                let url = if let Some(url_start) = msg.find(" \"") {
                    if let Some(url_end) = msg[url_start + 2..].find("\"") {
                        &msg[url_start + 2..url_start + 2 + url_end]
                    } else {
                        ""
                    }
                } else {
                    ""
                };
                let view_width = app.app_view.viewport_width(Panel::RequestDetail);
                // Include the method in the displayed text
                let text = format!("{} {}", method, url)
                    .chars()
                    .take(view_width - 10)
                    .collect::<String>();
                Span::raw(text)
            } else {
                Span::raw("".to_string())
            };

            let viewport_height = app.app_view.viewport_height(Panel::RequestDetail);
            let detail_scroll_offset = app.app_view.get_scroll_offset(Panel::RequestDetail);

            let (display_lines, total_display_entries) = if app.simple_mode_enabled {
                // Filter logs for Simple Mode using format_simple_log_line
                let simple_lines: Vec<Line<'static>> = group
                    .entries
                    .iter()
                    .filter_map(|log| format_simple_log_line(&log.message))
                    .collect();
                let count = simple_lines.len();
                (simple_lines, count)
            } else {
                // Prepare lines for Normal Mode
                let normal_lines: Vec<Line<'static>> = group
                    .entries
                    .iter()
                    .map(|log| {
                        let message = if let Some(after_id) =
                            strip_ansi_for_parsing(&log.message).find(']')
                        {
                            let raw_message = &log.message[(after_id + 1)..].trim();
                            raw_message.to_string()
                        } else {
                            log.message.clone()
                        };
                        let spans = parse_ansi_colors(&message);
                        Line::from(spans)
                    })
                    .collect();
                let count = normal_lines.len();
                (normal_lines, count)
            };

            let start_idx = detail_scroll_offset.min(total_display_entries.saturating_sub(1));
            let visible_count =
                viewport_height.min(total_display_entries.saturating_sub(start_idx));

            for i in 0..visible_count {
                let idx = total_display_entries
                    .saturating_sub(1)
                    .saturating_sub(start_idx + i);
                if idx < display_lines.len() {
                    text.extend(Text::from(display_lines[idx].clone()));
                }
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
        let total_entries = if app.simple_mode_enabled {
            // Count only the lines that match the simple format
            group
                .entries
                .iter()
                .filter(|log| format_simple_log_line(&log.message).is_some())
                .count()
        } else {
            group.entries.len()
        };

        if total_entries == 0 {
            "0/0".to_string()
        } else {
            let detail_scroll_offset = app.app_view.get_scroll_offset(Panel::RequestDetail);
            let start_idx = (detail_scroll_offset + INDEX_OFFSET).min(total_entries);
            format!("{}-*/{}", start_idx.max(1), total_entries)
        }
    } else {
        "0/0".to_string()
    };

    let title_text = format!("[{}] {} ", scroll_info, title_span);
    let title_style = match app.app_view.focused_panel {
        Panel::RequestDetail => Style::default().fg(Color::Yellow),
        _ => Style::default().fg(Color::White),
    };
    let block = Block::default()
        .padding(Padding::new(1, 1, 1, 1))
        .title_alignment(ratatui::layout::Alignment::Left)
        .title(Span::styled(title_text, title_style))
        .borders(Borders::ALL)
        .border_style(border_style);

    if app.simple_mode_enabled {
        paragraph.block(block)
    } else {
        paragraph.block(block).wrap(Wrap { trim: true })
    }
}

pub fn build_log_stream_component(app: &App) -> Paragraph<'_> {
    let mut log_text = Text::default();
    let total_logs = app.state.all_logs.len();

    let viewport_height = app.app_view.viewport_height(Panel::LogStream);

    if total_logs > 0 {
        let visible_logs = app.get_visible_all_logs(viewport_height);

        for log in visible_logs.iter() {
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
    } else if app.simple_mode_enabled {
        " SIMPLE MODE (press 's' to exit) | j/k: scroll | Tab/Shift+Tab: panels | Ctrl+c: quit | m: copy mode "
    } else {
        " j/k: scroll | Ctrl+d/u: page | Tab/Shift+Tab: panels | Ctrl+c: quit | m: copy | s: simple mode "
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

    let title_text = format!("[{}] ", scroll_info);
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

    let title_text = format!("[{}] ", scroll_info);
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
