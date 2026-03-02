use crate::app::App;
use crate::app_state::StatusType;
use crate::layout::Panel;
use crate::log_parser::strip_ansi_for_parsing;
use crate::simple_formatter::{format_simple_log_line, parse_ansi_colors};
use crate::sql_info::{QueryType, SqlQueryInfo};
use crate::theme::{ColorExt, THEME};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph, Wrap},
};

const INDEX_OFFSET: usize = 1;

pub fn build_list_component(app: &App) -> List<'_> {
    let visible_requests = app.visible_request_ids();
    let total_visible = visible_requests.len();

    let mut items = Vec::with_capacity(total_visible);

    let viewport_height = app.app_view.viewport_height(Panel::RequestList);
    let current_offset = app.app_view.get_scroll_offset(Panel::RequestList);
    let visible_count = viewport_height.min(total_visible.saturating_sub(current_offset));

    for &(original_index, request_id) in visible_requests
        .iter()
        .skip(current_offset)
        .take(visible_count)
    {
        let Some(group) = app.state.logs_by_request_id.get(request_id) else {
            continue;
        };
        let time_str = group.first_timestamp.format("%H:%M").to_string();

        let finished = group.finished;

        let status_color = if finished {
            group.status_type.to_color()
        } else {
            THEME.default
        };

        let duration_str = match group.duration_ms {
            Some(ms) => format!("{:>4}ms ", ms),
            None => " ---ms ".to_string(),
        };
        let duration_color = match group.duration_ms {
            Some(ms) if ms >= 3000 => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            Some(ms) if ms >= 500 => Style::default().fg(Color::Yellow),
            _ => Style::default().fg(Color::Cyan),
        };

        let content = Line::from(vec![
            Span::raw(format!("{} ", time_str)),
            Span::styled(duration_str, duration_color),
            Span::styled(group.title.as_str(), status_color),
        ]);

        let style = if original_index == app.state.selected_index {
            status_color.style_with_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else if finished {
            THEME.default.style().fg(status_color)
        } else {
            THEME.default.style()
        };

        items.push(ListItem::new(content).style(style));
    }

    let border_style = match app.app_view.focused_panel {
        Panel::RequestList => THEME.active_border,
        _ => THEME.border,
    };

    let total_requests = app.state.log_group_count();
    let scroll_info = if total_visible == 0 {
        "0/0".to_string()
    } else if app.filtered_indices.is_some() {
        format!("{}/{}", total_visible, total_requests)
    } else {
        let start_idx = current_offset + INDEX_OFFSET;
        let end_idx = (start_idx + visible_count - INDEX_OFFSET).min(total_visible);
        format!("{}-{}/{}", start_idx, end_idx, total_requests)
    };

    let is_list_search = matches!(app.search_mode, Some(crate::app::SearchTarget::RequestList));
    let title_text = if is_list_search || app.filtered_indices.is_some() {
        format!("[{}] /{}", scroll_info, app.search_query)
    } else {
        format!("[{}]", scroll_info)
    };

    let title_style = match app.app_view.focused_panel {
        Panel::RequestList => THEME.default.style_with_modifier(Modifier::BOLD),
        _ => THEME.default.style(),
    };

    let borders = if app.copy_mode_enabled {
        Borders::TOP | Borders::BOTTOM
    } else {
        Borders::ALL
    };

    let mut block = Block::default()
        .borders(borders)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .padding(Padding::new(1, 1, 1, 1))
        .title(Span::styled(title_text, title_style));

    if is_list_search {
        let search_display = format!(" /{}_ ", app.search_query);
        block = block.title_bottom(
            Line::from(Span::styled(
                search_display,
                Style::default().fg(Color::Yellow),
            ))
            .alignment(ratatui::layout::Alignment::Left),
        );
    }

    List::new(items).block(block)
}

pub fn build_detail_component(app: &App) -> Paragraph<'_> {
    let (title_span, log_text, total_entries) = build_detail_content(app);

    let border_style = match app.app_view.focused_panel {
        Panel::RequestDetail => THEME.active_border,
        _ => THEME.border,
    };

    let scroll_info = build_detail_scroll_info(app, total_entries);
    let title_text = format!("[{}] {} ", scroll_info, title_span);
    let status = app
        .state
        .selected_group()
        .map_or(StatusType::Unknown, |g| g.status_type);
    let title_style = status.to_color().style_with_modifier(Modifier::BOLD);

    let borders = if app.copy_mode_enabled {
        Borders::TOP | Borders::BOTTOM
    } else {
        Borders::ALL
    };

    let bottom_line = build_detail_bottom_bar(app);

    let block = Block::default()
        .padding(Padding::new(1, 1, 1, 1))
        .title_alignment(ratatui::layout::Alignment::Left)
        .title(Span::styled(title_text, title_style))
        .title_bottom(bottom_line)
        .borders(borders)
        .border_style(border_style);

    let paragraph = Paragraph::new(log_text);
    if app.simple_mode_enabled {
        paragraph.block(block)
    } else {
        paragraph.block(block).wrap(Wrap { trim: true })
    }
}

fn build_detail_title(app: &App, group: &crate::app_state::LogGroup) -> Span<'static> {
    let entry = group.entries.iter().find(|entry| {
        let msg = &entry.message;
        msg.contains("Started GET")
            || msg.contains("Started POST")
            || msg.contains("Started PUT")
            || msg.contains("Started PATCH")
            || msg.contains("Started DELETE")
            || msg.contains("Started HEAD")
            || msg.contains("Started OPTIONS")
            || msg.contains("Started TRACE")
    });

    let Some(entry) = entry else {
        return Span::raw("");
    };

    let msg = strip_ansi_for_parsing(&entry.message);
    let method = msg
        .split_whitespace()
        .skip_while(|&s| s != "Started")
        .nth(1)
        .unwrap_or("");

    let url = msg
        .find(" \"")
        .and_then(|start| {
            msg[start + 2..]
                .find('"')
                .map(|end| &msg[start + 2..start + 2 + end])
        })
        .unwrap_or("");

    let view_width = app.app_view.viewport_width(Panel::RequestDetail);
    let text = format!("{} {}", method, url)
        .chars()
        .take(view_width.saturating_sub(10))
        .collect::<String>();
    Span::raw(text)
}

fn build_detail_log_line(
    log: &crate::app_state::LogEntry,
    sql_info: &SqlQueryInfo,
    detail_query: &str,
    simple_mode: bool,
) -> Option<Line<'static>> {
    if simple_mode {
        format_simple_log_line(&log.message)
            .map(|line| highlight_n_plus_one_tables(line, sql_info))
            .map(|line| highlight_search_matches(line, detail_query))
    } else {
        let message = if let Some(after_id) = log.message.find(']') {
            log.message[(after_id + 1)..].trim().to_string()
        } else {
            log.message.clone()
        };
        let spans = parse_ansi_colors(&message);
        let line = Line::from(spans);
        let line = highlight_n_plus_one_tables(line, sql_info);
        Some(highlight_search_matches(line, detail_query))
    }
}

fn build_detail_content(app: &App) -> (Span<'static>, Text<'static>, usize) {
    let Some(group) = app.state.selected_group() else {
        return (Span::raw("Logs"), Text::from("Waiting for logs..."), 0);
    };

    let title_span = build_detail_title(app, group);
    let sql_info = &group.sql_query_info;
    let detail_query = &app.detail_search_query;
    let simple_mode = app.simple_mode_enabled;

    let viewport_height = app.app_view.viewport_height(Panel::RequestDetail);
    let scroll_offset = app.app_view.get_scroll_offset(Panel::RequestDetail);

    // Entries are stored newest-first (push_front), so reverse for display
    let mut text = Text::default();
    let total = if simple_mode {
        // Collect filtered lines once in chronological order
        let all_lines: Vec<Line<'static>> = group
            .entries
            .iter()
            .rev()
            .filter_map(|log| format_simple_log_line(&log.message))
            .collect();
        let total = all_lines.len();
        let start_idx = scroll_offset.min(total.saturating_sub(1));
        let visible_count = viewport_height.min(total.saturating_sub(start_idx));

        for line in all_lines.into_iter().skip(start_idx).take(visible_count) {
            let line = highlight_n_plus_one_tables(line, sql_info);
            let line = highlight_search_matches(line, detail_query);
            text.extend(Text::from(line));
        }
        total
    } else {
        let total = group.entries.len();
        let start_idx = scroll_offset.min(total.saturating_sub(1));
        let visible_count = viewport_height.min(total.saturating_sub(start_idx));

        for i in 0..visible_count {
            let idx = total.saturating_sub(1).saturating_sub(start_idx + i);
            if let Some(log) = group.entries.get(idx)
                && let Some(line) = build_detail_log_line(log, sql_info, detail_query, false)
            {
                text.extend(Text::from(line));
            }
        }
        total
    };

    (title_span, text, total)
}

fn build_detail_scroll_info(app: &App, total_entries: usize) -> String {
    if total_entries == 0 {
        "0/0".to_string()
    } else {
        let scroll_offset = app.app_view.get_scroll_offset(Panel::RequestDetail);
        let start_idx = (scroll_offset + INDEX_OFFSET).min(total_entries);
        format!("{}-*/{}", start_idx.max(1), total_entries)
    }
}

fn build_detail_bottom_bar(app: &App) -> Line<'static> {
    let is_detail_search =
        matches!(app.search_mode, Some(crate::app::SearchTarget::DetailLog));
    let has_detail_query = !app.detail_search_query.is_empty();

    if is_detail_search || has_detail_query {
        let search_display = if is_detail_search {
            format!(" /{}_ ", app.detail_search_query)
        } else {
            format!(" /{} ", app.detail_search_query)
        };
        Line::from(vec![
            Span::styled(search_display, Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("  {}", help_text(app)),
                Style::default().fg(Color::DarkGray),
            ),
        ])
        .alignment(ratatui::layout::Alignment::Left)
    } else {
        Line::from(vec![Span::styled(
            help_text(app),
            Style::default().fg(Color::DarkGray),
        )])
        .alignment(ratatui::layout::Alignment::Right)
    }
}

fn help_text(app: &App) -> String {
    if app.copy_mode_enabled {
        let panel_name = match app.app_view.focused_panel {
            Panel::RequestList => "RequestList",
            Panel::RequestDetail => "RequestDetail",
            Panel::SqlInfo => "SqlInfo",
        };
        return format!(" COPY MODE [{}] (Tab: switch panel | m: exit) ", panel_name);
    }
    if app.simple_mode_enabled {
        " SIMPLE MODE (press 's' to exit) | j/k | Tab/Shift+Tab | Ctrl+c | m: copy | /: search"
            .to_string()
    } else {
        " j/k | Ctrl+d/u | Tab/Shift+Tab | Ctrl+c | m: copy | s: simple | /: search".to_string()
    }
}

pub fn build_sql_component(app: &App) -> Paragraph<'_> {
    let border_style = match app.app_view.focused_panel {
        Panel::SqlInfo => THEME.active_border,
        _ => THEME.border,
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
                let mut spans = vec![
                    Span::styled(
                        format!("{}: ", table),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(count.to_string()),
                ];
                if sql_info.is_n_plus_one(table) {
                    spans.push(Span::styled(
                        " N+1?",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
                text.extend(Text::from(Line::from(spans)));
            }
        }
    }

    let scroll_info = if let Some(group) = app.state.selected_group() {
        let total_queries = group.sql_query_info.total_queries();
        if total_queries == 0 {
            "0/0".to_string()
        } else {
            total_queries.to_string()
        }
    } else {
        "0/0".to_string()
    };

    let borders = if app.copy_mode_enabled {
        Borders::TOP | Borders::BOTTOM
    } else {
        Borders::ALL
    };

    let title_text = format!("[{}] ", scroll_info);
    let block = Block::default()
        .borders(borders)
        .border_style(border_style)
        .padding(Padding::new(1, 1, 0, 0))
        .title(title_text);

    let sql_scroll_offset = app.app_view.get_scroll_offset(Panel::SqlInfo);

    Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: true })
        .scroll((sql_scroll_offset as u16, 0))
}

fn highlight_n_plus_one_tables<'a>(line: Line<'a>, sql_info: &SqlQueryInfo) -> Line<'a> {
    let n1_tables: Vec<&String> = sql_info
        .select_per_table
        .iter()
        .filter(|(t, _)| sql_info.is_n_plus_one(t))
        .map(|(t, _)| t)
        .collect();

    if n1_tables.is_empty() {
        return line;
    }

    let highlight_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let mut new_spans: Vec<Span<'a>> = Vec::new();

    for span in line.spans {
        let content: &str = &span.content;
        let mut remaining = content.to_string();
        let mut parts: Vec<Span<'a>> = Vec::new();
        let mut found = true;

        while found {
            found = false;
            // Find the earliest matching table name in the remaining text
            let mut earliest: Option<(usize, &String)> = None;
            for table in &n1_tables {
                if let Some(pos) = remaining.find(table.as_str())
                    && (earliest.is_none() || pos < earliest.unwrap().0)
                {
                    earliest = Some((pos, table));
                }
            }

            if let Some((pos, table)) = earliest {
                found = true;
                if pos > 0 {
                    parts.push(Span::styled(remaining[..pos].to_string(), span.style));
                }
                parts.push(Span::styled(table.to_string(), highlight_style));
                remaining = remaining[pos + table.len()..].to_string();
            }
        }

        if !remaining.is_empty() {
            parts.push(Span::styled(remaining, span.style));
        }

        if parts.is_empty() {
            new_spans.push(span);
        } else {
            new_spans.extend(parts);
        }
    }

    Line::from(new_spans)
}

fn highlight_search_matches<'a>(line: Line<'a>, query: &str) -> Line<'a> {
    if query.is_empty() {
        return line;
    }

    let highlight_style = Style::default()
        .bg(Color::Yellow)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);

    let query_lower = query.to_lowercase();
    let mut new_spans: Vec<Span<'a>> = Vec::new();

    for span in line.spans {
        let content: &str = &span.content;
        let content_lower = content.to_lowercase();
        let mut last_end = 0;
        let mut parts: Vec<Span<'a>> = Vec::new();

        for (start, matched) in content_lower.match_indices(&query_lower) {
            let end = start + matched.len();
            // UTF-8境界チェック（小文字化でバイト長が変わるケースに対応）
            if end > content.len()
                || !content.is_char_boundary(start)
                || !content.is_char_boundary(end)
            {
                continue;
            }
            if start > last_end {
                parts.push(Span::styled(content[last_end..start].to_string(), span.style));
            }
            parts.push(Span::styled(
                content[start..end].to_string(),
                highlight_style,
            ));
            last_end = end;
        }

        if parts.is_empty() {
            new_spans.push(span);
        } else {
            if last_end < content.len() {
                parts.push(Span::styled(content[last_end..].to_string(), span.style));
            }
            new_spans.extend(parts);
        }
    }

    Line::from(new_spans)
}
