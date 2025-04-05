use chrono::{DateTime, Local};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
};
use ratatui::layout::Rect;
use std::{
    collections::{BTreeMap, HashMap},
    io,
    sync::mpsc,
    time::{Duration, Instant},
};
use tracing::debug;

use super::ui;
use crate::layout::{LayoutInfo, Panel};
use crate::sql_info::SqlQueryInfo;

pub struct AppView {
    pub focused_panel: Panel,
    pub scroll_offsets: HashMap<Panel, usize>,
    pub layout_info: LayoutInfo,
}

impl AppView {
    const VIEW_PADDING: u16 = 4;

    pub fn new() -> Self {
        let mut scroll_offsets = HashMap::new();
        scroll_offsets.insert(Panel::RequestList, 0);
        scroll_offsets.insert(Panel::RequestDetail, 0);
        scroll_offsets.insert(Panel::LogStream, 0);
        scroll_offsets.insert(Panel::SqlInfo, 0);

        Self {
            focused_panel: Panel::RequestList,
            scroll_offsets,
            layout_info: LayoutInfo::new(),
        }
    }

    pub fn get_scroll_offset(&self, panel: Panel) -> usize {
        *self.scroll_offsets.get(&panel).unwrap_or(&0)
    }

    pub fn set_scroll_offset(&mut self, panel: Panel, offset: usize) {
        if let Some(current) = self.scroll_offsets.get_mut(&panel) {
            *current = offset;
        }
    }

    pub fn apply_scroll(&mut self, panel: Panel, delta: i8, max_scroll: usize) {
        let current = self.get_scroll_offset(panel);

        if delta > 0 {
            let new_offset = (current + delta as usize).min(max_scroll);
            self.set_scroll_offset(panel, new_offset);
        } else if delta < 0 {
            let new_offset = current.saturating_sub(delta.unsigned_abs() as usize);
            self.set_scroll_offset(panel, new_offset);
        }
    }

    pub fn get_viewport_height(&self, panel: Panel) -> usize {
        let region = self.layout_info.get_region(panel);

        match panel {
            Panel::RequestDetail => region.height.saturating_sub(Self::VIEW_PADDING) as usize,
            Panel::LogStream => region.height.saturating_sub(Self::VIEW_PADDING) as usize,
            Panel::SqlInfo => region.height.saturating_sub(Self::VIEW_PADDING) as usize,
            Panel::RequestList => region.height.saturating_sub(Self::VIEW_PADDING) as usize,
        }
    }
}

pub struct App {
    pub app_view: AppView,
    pub logs_by_request_id: BTreeMap<String, LogGroup>,
    pub selected_index: usize,
    pub request_ids: Vec<String>,
    pub first_timestamps: HashMap<String, DateTime<Local>>,
    pub all_logs: Vec<LogEntry>,
    pub copy_mode_enabled: bool,
}

pub struct LogGroup {
    pub title: String,
    pub entries: Vec<LogEntry>,
    pub finished: bool,
    pub sql_query_info: SqlQueryInfo,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub request_id: String,
    pub message: String,
}
static SCROLL_PAGE_SIZE: i8 = 5;

impl App {
    pub fn new() -> Self {
        Self {
            app_view: AppView::new(),
            logs_by_request_id: BTreeMap::new(),
            selected_index: 0,
            request_ids: Vec::new(),
            first_timestamps: HashMap::new(),
            all_logs: Vec::new(),
            copy_mode_enabled: false,
        }
    }

    pub fn selected_request_id(&self) -> Option<&String> {
        self.request_ids.get(self.selected_index)
    }

    pub fn run<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
        rx: mpsc::Receiver<String>,
    ) -> io::Result<()> {
        let mut last_tick = Instant::now();

        loop {
            terminal.draw(|f| {
                let layout_info = crate::layout::calculate_layout(f.area());
                self.app_view.layout_info = layout_info.clone();
                ui::render(f, self);
            })?;

            if let Ok(line) = rx.try_recv() {
                if let Some(log_entry) = crate::log_parser::parse(&line) {
                    self.add_log_entry(log_entry);
                }
            }

            let timeout = Duration::from_millis(16)
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_millis(16));

            let poll_result = crossterm::event::poll(timeout);
            if let Err(e) = &poll_result {
                debug!("Poll error: {:?}", e);
                continue;
            }

            if poll_result.unwrap() {
                let event_result = event::read();
                if let Err(e) = &event_result {
                    debug!("Event read error: {:?}", e);
                    continue;
                }

                match event_result.unwrap() {
                    Event::Key(key) => match key.code {
                        KeyCode::Char('c')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            return Ok(());
                        }
                        KeyCode::BackTab => self.toggle_focus_reverse(),
                        KeyCode::Tab => self.toggle_focus(),
                        KeyCode::Char(' ') => self.jump_to_latest(),
                        KeyCode::Char('a') => self.toggle_all_logs_panel(),
                        KeyCode::Char('m') | KeyCode::Char('M') => self.toggle_copy_mode()?,
                        KeyCode::Char('d')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            match self.app_view.focused_panel {
                                Panel::RequestList => {
                                    self.next_request();
                                    self.next_request();
                                    self.next_request();
                                }
                                Panel::RequestDetail => {
                                    self.apply_scroll_to(Panel::RequestDetail, SCROLL_PAGE_SIZE)
                                }
                                Panel::LogStream => {
                                    self.apply_scroll_to(Panel::LogStream, SCROLL_PAGE_SIZE)
                                }
                                Panel::SqlInfo => {
                                    self.apply_scroll_to(Panel::SqlInfo, SCROLL_PAGE_SIZE)
                                }
                            }
                        }
                        KeyCode::Char('u')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            match self.app_view.focused_panel {
                                Panel::RequestList => {
                                    self.previous_request();
                                    self.previous_request();
                                    self.previous_request();
                                }
                                Panel::RequestDetail => {
                                    self.apply_scroll_to(Panel::RequestDetail, -SCROLL_PAGE_SIZE)
                                }
                                Panel::LogStream => {
                                    self.apply_scroll_to(Panel::LogStream, -SCROLL_PAGE_SIZE)
                                }
                                Panel::SqlInfo => {
                                    self.apply_scroll_to(Panel::SqlInfo, -SCROLL_PAGE_SIZE)
                                }
                            }
                        }
                        _ => match self.app_view.focused_panel {
                            Panel::RequestList => match key.code {
                                KeyCode::Char('j') | KeyCode::Down => self.next_request(),
                                KeyCode::Char('k') | KeyCode::Up => self.previous_request(),
                                _ => {}
                            },
                            Panel::RequestDetail | Panel::LogStream | Panel::SqlInfo => {
                                match key.code {
                                    KeyCode::Char('j') | KeyCode::Down => {
                                        self.apply_scroll_to(self.app_view.focused_panel, 1)
                                    }
                                    KeyCode::Char('k') | KeyCode::Up => {
                                        self.apply_scroll_to(self.app_view.focused_panel, -1)
                                    }
                                    KeyCode::PageDown => self.apply_scroll_to(
                                        self.app_view.focused_panel,
                                        SCROLL_PAGE_SIZE,
                                    ),
                                    KeyCode::PageUp => self.apply_scroll_to(
                                        self.app_view.focused_panel,
                                        -SCROLL_PAGE_SIZE,
                                    ),
                                    _ => {}
                                }
                            }
                        },
                    },
                    Event::Mouse(mouse_event) if !self.copy_mode_enabled => {
                        let layout_info = self.app_view.layout_info.clone();
                        self.handle_mouse_event(mouse_event, &layout_info);
                    }
                    _ => {}
                }
            }

            if last_tick.elapsed() >= Duration::from_millis(16) {
                last_tick = Instant::now();
            }
        }
    }

    fn adjust_scroll_for_index(&mut self, panel: Panel, index: usize) {
        let viewport_height = self.app_view.get_viewport_height(panel);
        let current_offset = self.app_view.get_scroll_offset(panel);

        if index < current_offset {
            self.app_view.set_scroll_offset(panel, index);
        } else if index >= current_offset + viewport_height {
            self.app_view
                .set_scroll_offset(panel, index.saturating_sub(viewport_height - 1));
        }
    }

    fn select_request(&mut self, index: usize) {
        if index < self.request_ids.len() {
            self.selected_index = index;
            self.app_view.set_scroll_offset(Panel::RequestDetail, 0);
            self.adjust_scroll_for_index(Panel::RequestList, self.selected_index);
        }
    }

    pub fn next_request(&mut self) {
        if self.request_ids.is_empty() {
            return;
        }

        if self.selected_index < self.request_ids.len() - 1 {
            self.select_request(self.selected_index + 1);
        }
    }

    pub fn previous_request(&mut self) {
        if self.request_ids.is_empty() {
            return;
        }

        if self.selected_index > 0 {
            self.select_request(self.selected_index - 1);
        }
    }

    fn apply_scroll_to(&mut self, panel: Panel, n: i8) {
        let max_scroll = match panel {
            Panel::RequestDetail => self.get_max_detail_scroll(),
            Panel::LogStream => self.get_max_stream_scroll(),
            Panel::SqlInfo => self.get_max_sql_scroll(),
            _ => 0,
        };

        self.app_view.apply_scroll(panel, n, max_scroll);
    }

    fn get_max_detail_scroll(&self) -> usize {
        if let Some(group) = self.selected_group() {
            let entry_count = group.entries.len();
            let viewport_height = self.app_view.get_viewport_height(Panel::RequestDetail);
            entry_count.saturating_sub(viewport_height).max(0)
        } else {
            0
        }
    }

    fn get_max_sql_scroll(&self) -> usize {
        if let Some(group) = self.selected_group() {
            let total_lines = group.sql_query_info.display_line_count();
            let viewport_height = self.app_view.get_viewport_height(Panel::SqlInfo);

            total_lines.saturating_sub(viewport_height).max(0)
        } else {
            0
        }
    }

    fn get_max_stream_scroll(&self) -> usize {
        let log_count = self.all_logs.len();
        let viewport_height = self.app_view.get_viewport_height(Panel::LogStream);
        log_count.saturating_sub(viewport_height).max(0)
    }

    pub fn get_visible_logs(&self, viewport_height: usize) -> Vec<&LogEntry> {
        let total_logs = self.all_logs.len();

        if total_logs == 0 {
            return Vec::new();
        }

        let start_idx = self
            .app_view
            .get_scroll_offset(Panel::LogStream)
            .min(total_logs.saturating_sub(1));
        let visible_count = viewport_height.min(total_logs.saturating_sub(start_idx));
        let mut result = Vec::with_capacity(visible_count);

        for i in 0..visible_count {
            let idx = start_idx + i;
            if idx < total_logs {
                result.push(&self.all_logs[idx]);
            }
        }

        result
    }

    pub fn add_log_entry(&mut self, log_entry: LogEntry) {
        if log_entry.request_id.is_empty() {
            self.all_logs.push(log_entry);
            self.auto_scroll_if_needed();
            return;
        }

        let request_id = log_entry.request_id.clone();
        let message = &log_entry.message;
        let is_new_request = !self.logs_by_request_id.contains_key(&request_id);

        let process_log_message = |message: &str, group: &mut LogGroup| {
            if message.contains("Started ") {
                if let Some(start_pos) = message.find("Started ") {
                    group.title = message[(start_pos + 8)..].to_string();
                } else {
                    group.title = message.to_string();
                }
            }

            if message.contains("Completed ") {
                group.finished = true;
            }

            crate::sql_info::SqlQueryInfo::from_message(message)
        };

        if is_new_request {
            self.request_ids.insert(0, request_id.clone());
            self.first_timestamps
                .insert(request_id.clone(), log_entry.timestamp);

            if self.request_ids.len() == 1 {
                self.selected_index = 0;
            } else {
                // don't move
                self.selected_index += 1;
            }

            if matches!(self.app_view.focused_panel, Panel::RequestList) {
                self.adjust_scroll_for_index(Panel::RequestList, self.selected_index);
            }

            let mut new_group = LogGroup {
                title: "...".to_string(),
                entries: Vec::with_capacity(8),
                finished: false,
                sql_query_info: crate::sql_info::SqlQueryInfo::new(),
            };

            if let Some(sql_info) = process_log_message(message, &mut new_group) {
                new_group.sql_query_info = sql_info;
            }

            new_group.entries.push(log_entry.clone());
            self.logs_by_request_id.insert(request_id, new_group);
        } else {
            let group = self.logs_by_request_id.get_mut(&request_id).unwrap();

            if let Some(new_sql_info) = process_log_message(message, group) {
                group.sql_query_info.merge(&new_sql_info);
            }

            group.entries.insert(0, log_entry.clone());
        }

        self.all_logs.push(log_entry);
        self.auto_scroll_if_needed();
    }

    pub fn selected_group(&self) -> Option<&LogGroup> {
        let request_id = self.selected_request_id()?;
        self.logs_by_request_id.get(request_id)
    }

    pub fn toggle_focus(&mut self) {
        self.app_view.focused_panel = match self.app_view.focused_panel {
            Panel::RequestList => Panel::RequestDetail,
            Panel::RequestDetail => Panel::LogStream,
            Panel::LogStream => Panel::SqlInfo,
            Panel::SqlInfo => Panel::RequestList,
        };
    }

    pub fn toggle_focus_reverse(&mut self) {
        self.app_view.focused_panel = match self.app_view.focused_panel {
            Panel::RequestList => Panel::SqlInfo,
            Panel::RequestDetail => Panel::RequestList,
            Panel::LogStream => Panel::RequestDetail,
            Panel::SqlInfo => Panel::LogStream,
        };
    }

    pub fn jump_to_latest(&mut self) {
        if !self.request_ids.is_empty() {
            self.select_request(0);
        }
    }

    pub fn toggle_all_logs_panel(&mut self) {
        self.app_view.focused_panel = match self.app_view.focused_panel {
            Panel::LogStream => Panel::RequestList,
            _ => Panel::LogStream,
        };
    }

    fn handle_mouse_event(&mut self, mouse_event: event::MouseEvent, layout_info: &LayoutInfo) {
        let x = mouse_event.column;
        let y = mouse_event.row;
        match mouse_event.kind {
            event::MouseEventKind::ScrollDown | event::MouseEventKind::ScrollUp => {
                if let Some(panel) = get_panel_at_point(x, y, layout_info) {
                    let scroll_delta = if let event::MouseEventKind::ScrollDown = mouse_event.kind {
                        1
                    } else {
                        -1
                    };

                    if panel == Panel::RequestList {
                        if scroll_delta > 0 {
                            self.next_request();
                        } else {
                            self.previous_request();
                        }
                    } else {
                        self.apply_scroll_to(panel, scroll_delta);
                    }
                }
                return;
            }

            event::MouseEventKind::Down(event::MouseButton::Left) => {
                if let Some(panel) = get_panel_at_point(x, y, layout_info) {
                    self.app_view.focused_panel = panel;

                    if panel == Panel::RequestList {
                        let row_in_list = y.saturating_sub(layout_info.request_list_region().y + 2);
                        let current_offset = self.app_view.get_scroll_offset(Panel::RequestList);
                        let clicked_index = current_offset + row_in_list as usize;

                        if clicked_index < self.request_ids.len() {
                            self.select_request(clicked_index);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn toggle_copy_mode(&mut self) -> io::Result<()> {
        self.copy_mode_enabled = !self.copy_mode_enabled;

        let mut stdout = io::stdout();
        if self.copy_mode_enabled {
            execute!(stdout, crossterm::event::DisableMouseCapture)?;
        } else {
            execute!(stdout, crossterm::event::EnableMouseCapture)?;
        }

        Ok(())
    }

    fn auto_scroll_if_needed(&mut self) {
        let is_all_logs_focused = matches!(self.app_view.focused_panel, Panel::LogStream);
        let max_stream_scroll = self.get_max_stream_scroll();
        let all_logs_offset = self.app_view.get_scroll_offset(Panel::LogStream);
        let at_bottom = all_logs_offset >= max_stream_scroll.saturating_sub(1);

        if is_all_logs_focused || at_bottom {
            self.app_view
                .set_scroll_offset(Panel::LogStream, self.get_max_stream_scroll());
        }
    }
}

fn is_in_region(x: u16, y: u16, area: &Rect) -> bool {
    x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
}

fn get_panel_at_point(x: u16, y: u16, layout_info: &LayoutInfo) -> Option<Panel> {
    if is_in_region(x, y, &layout_info.request_list_region()) {
        Some(Panel::RequestList)
    } else if is_in_region(x, y, &layout_info.request_detail_region()) {
        Some(Panel::RequestDetail)
    } else if is_in_region(x, y, &layout_info.sql_info_region()) {
        Some(Panel::SqlInfo)
    } else if is_in_region(x, y, &layout_info.log_stream_region()) {
        Some(Panel::LogStream)
    } else {
        None
    }
}
