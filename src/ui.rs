use crate::app::App;
use crate::layout::Panel;
use crate::log_parser::strip_ansi_for_parsing;
use crate::sql_info::QueryType;
use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph, Wrap},
};

pub fn render(f: &mut Frame, app: &mut App) {
    let layout_info = &app.app_view.layout_info;
    let request_list_region = layout_info.request_list_region();
    let request_detail_region = layout_info.request_detail_region();
    let log_stream_region = layout_info.log_stream_region();
    let sql_info_region = layout_info.sql_info_region();

    let request_list = list_component(app);
    f.render_widget(request_list, request_list_region);
    let detail_panel = detail_component(app);
    f.render_widget(detail_panel, request_detail_region);
    let log_stream = log_stream_component(app);
    f.render_widget(log_stream, log_stream_region);
    let sql_panel: Paragraph<'_> = sql_component(app);
    f.render_widget(sql_panel, sql_info_region);
}

pub fn list_component(app: &mut App) -> List<'_> {
    let mut items = Vec::with_capacity(app.request_ids.len());

    let viewport_height = app.app_view.get_viewport_height(Panel::RequestList);
    let new_offset = if app.request_ids.len() > viewport_height {
        let current_offset = app.app_view.get_scroll_offset(Panel::RequestList);

        if app.selected_index < current_offset {
            app.selected_index
        } else if app.selected_index >= current_offset + viewport_height {
            app.selected_index.saturating_sub(viewport_height - 1)
        } else {
            current_offset
        }
    } else {
        0
    };

    app.app_view
        .set_scroll_offset(Panel::RequestList, new_offset);

    let visible_count = viewport_height.min(app.request_ids.len().saturating_sub(new_offset));
    let end_idx = new_offset + visible_count;

    for index in new_offset..end_idx {
        if index >= app.request_ids.len() {
            break;
        }

        let request_id = &app.request_ids[index];
        let time_str = app.first_timestamps.get(request_id).map_or_else(
            || "??:??:??".to_string(),
            |ts| ts.format("%H:%M:%S").to_string(),
        );

        let group = app.logs_by_request_id.get(request_id).unwrap();
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

        let style = if index == app.selected_index {
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

    let is_active = matches!(app.app_view.focused_panel, Panel::RequestList);
    let border_style = if is_active {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
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

pub fn detail_component(app: &mut App) -> Paragraph<'_> {
    let (title_span, log_text) = match app.selected_group() {
        None => (Span::raw("Logs"), Text::from("Waiting for logs...")),
        Some(group) => {
            let is_finished = group.finished;
            let status_text = if is_finished { "Completed" } else { "Running" };

            let title_span = Span::styled(
                format!(" Status: {} ", status_text),
                Style::default()
                    .fg(if is_finished {
                        Color::Green
                    } else {
                        Color::Yellow
                    })
                    .add_modifier(Modifier::BOLD),
            );

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

            let viewport_height = app.app_view.get_viewport_height(Panel::RequestDetail);
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

    let is_active = matches!(app.app_view.focused_panel, Panel::RequestDetail);
    let border_style = if is_active {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let scroll_info = if let Some(group) = app.selected_group() {
        let total_entries = group.entries.len();
        if total_entries == 0 {
            "0/0".to_string()
        } else {
            let detail_scroll_offset = app.app_view.get_scroll_offset(Panel::RequestDetail);
            let start_idx = detail_scroll_offset + 1;
            let viewport_height = app.app_view.get_viewport_height(Panel::RequestDetail);
            let end_idx = (start_idx + viewport_height - 1).min(total_entries);
            format!("{}-{}/{}", start_idx, end_idx, total_entries)
        }
    } else {
        "0/0".to_string()
    };

    let title_text = format!(" [{}] ", scroll_info);

    let block = Block::default()
        .padding(Padding::new(1, 1, 1, 1))
        .title_alignment(ratatui::layout::Alignment::Right)
        .title(title_span)
        .title(Span::styled(
            title_text,
            Style::default().fg(if is_active {
                Color::Yellow
            } else {
                Color::White
            }),
        ))
        .borders(Borders::ALL)
        .border_style(border_style);

    Paragraph::new(log_text)
        .block(block)
        .wrap(Wrap { trim: true })
}

pub fn log_stream_component(app: &mut App) -> Paragraph<'_> {
    let mut log_text = Text::default();
    let total_logs = app.all_logs.len();

    let viewport_height = app.app_view.get_viewport_height(Panel::LogStream);

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
        " j/k: scroll | Ctrl+d/u: page | Tab/Shift+Tab: panels | Ctrl+c: quit | m: copy mode "
    };

    let scroll_info = if total_logs == 0 {
        "0/0".to_string()
    } else {
        let all_scroll_offset = app.app_view.get_scroll_offset(Panel::LogStream);
        let start_idx = all_scroll_offset + 1;
        let end_idx = (start_idx + viewport_height - 1).min(total_logs);
        format!("{}-{}/{}", start_idx, end_idx, total_logs)
    };

    let is_active = matches!(app.app_view.focused_panel, Panel::LogStream);
    let border_style = if is_active {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title_text = format!(" All Logs Stream [{}] ", scroll_info);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .padding(Padding::new(1, 1, 1, 1))
        .title(title_text)
        .title_alignment(ratatui::layout::Alignment::Left)
        .title_bottom(
            Line::from(vec![Span::styled(
                copy_mode_text,
                Style::default()
                    .fg(if app.copy_mode_enabled {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    })
                    .add_modifier(if app.copy_mode_enabled {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            )])
            .alignment(ratatui::layout::Alignment::Right),
        );

    Paragraph::new(log_text)
        .block(block)
        .wrap(Wrap { trim: true })
        .alignment(ratatui::layout::Alignment::Left)
}

pub fn sql_component(app: &mut App) -> Paragraph<'_> {
    let is_active = matches!(app.app_view.focused_panel, Panel::SqlInfo);
    let border_style = if is_active {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let mut text = Text::default();

    if let Some(group) = app.selected_group() {
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

    let scroll_info = if let Some(group) = app.selected_group() {
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
