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

use super::ui;

pub struct App {
    pub logs_by_request_id: BTreeMap<String, LogGroup>,
    pub selected_index: usize,
    pub request_ids: Vec<String>,
    pub first_timestamps: HashMap<String, DateTime<Local>>,
    pub detail_scroll_offset: usize,
    pub all_scroll_offset: usize,
    pub sql_scroll_offset: usize,
    pub focused_panel: Panel,
    pub all_logs: Vec<LogEntry>,
    pub copy_mode_enabled: bool,
    pub layout_info: LayoutInfo,
    pub debug_text: String,
}

pub struct LogGroup {
    pub title: String,
    pub entries: Vec<LogEntry>,
    pub finished: bool,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub request_id: String,
    pub message: String,
}

pub enum Panel {
    List,
    Detail,
    AllLogs,
    SqlDetail,
}

#[derive(Default, Debug, Clone)]
pub struct LayoutInfo {
    pub left_region: Rect,
    pub right_region: Rect,
    pub bottom_region: Rect,
    pub sql_region: Rect,
}

static SCROLL_PAGE_SIZE: i8 = 5;

impl App {
    pub fn new() -> Self {
        Self {
            logs_by_request_id: BTreeMap::new(),
            selected_index: 0,
            request_ids: Vec::new(),
            first_timestamps: HashMap::new(),
            detail_scroll_offset: 0,
            all_scroll_offset: 0,
            sql_scroll_offset: 0,
            focused_panel: Panel::List,
            all_logs: Vec::new(),
            copy_mode_enabled: false,
            layout_info: LayoutInfo::default(),
            debug_text: String::new(),
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
                let layout_info = ui::ui(f, self);
                self.layout_info = layout_info;
            })?;

            if let Ok(line) = rx.try_recv() {
                if let Some(log_entry) = crate::parser::parse(&line) {
                    self.add_log_entry(log_entry);
                }
            }

            let timeout = Duration::from_millis(16)
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_millis(0));

            let poll_result = crossterm::event::poll(timeout);
            if let Err(e) = &poll_result {
                self.debug_text = format!("Poll error: {:?}", e);
                continue;
            }

            if poll_result.unwrap() {
                let event_result = event::read();
                if let Err(e) = &event_result {
                    self.debug_text = format!("Event read error: {:?}", e);
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
                            match self.focused_panel {
                                Panel::List => {
                                    self.next_request();
                                    self.next_request();
                                    self.next_request();
                                }
                                Panel::Detail => {
                                    self.apply_scroll_to(Panel::Detail, SCROLL_PAGE_SIZE)
                                }
                                Panel::AllLogs => {
                                    self.apply_scroll_to(Panel::AllLogs, SCROLL_PAGE_SIZE)
                                }
                                Panel::SqlDetail => {
                                    self.apply_scroll_to(Panel::SqlDetail, SCROLL_PAGE_SIZE)
                                }
                            }
                        }
                        KeyCode::Char('u')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            match self.focused_panel {
                                Panel::List => {
                                    self.previous_request();
                                    self.previous_request();
                                    self.previous_request();
                                }
                                Panel::Detail => {
                                    self.apply_scroll_to(Panel::Detail, -SCROLL_PAGE_SIZE)
                                }
                                Panel::AllLogs => {
                                    self.apply_scroll_to(Panel::AllLogs, -SCROLL_PAGE_SIZE)
                                }
                                Panel::SqlDetail => {
                                    self.apply_scroll_to(Panel::SqlDetail, -SCROLL_PAGE_SIZE)
                                }
                            }
                        }
                        _ => {
                            self.debug_text = format!("Key: {:?}", key);

                            match self.focused_panel {
                                Panel::List => match key.code {
                                    KeyCode::Char('j') | KeyCode::Down => self.next_request(),
                                    KeyCode::Char('k') | KeyCode::Up => self.previous_request(),
                                    _ => {}
                                },
                                Panel::Detail => match key.code {
                                    KeyCode::Char('j') | KeyCode::Down => {
                                        self.apply_scroll_to(Panel::Detail, 1)
                                    }
                                    KeyCode::Char('k') | KeyCode::Up => {
                                        self.apply_scroll_to(Panel::Detail, -1)
                                    }
                                    KeyCode::PageDown => {
                                        self.apply_scroll_to(Panel::Detail, SCROLL_PAGE_SIZE)
                                    }
                                    KeyCode::PageUp => {
                                        self.apply_scroll_to(Panel::Detail, -SCROLL_PAGE_SIZE)
                                    }
                                    _ => {}
                                },
                                Panel::AllLogs => match key.code {
                                    KeyCode::Char('j') | KeyCode::Down => {
                                        self.apply_scroll_to(Panel::AllLogs, 1)
                                    }
                                    KeyCode::Char('k') | KeyCode::Up => {
                                        self.apply_scroll_to(Panel::AllLogs, -1)
                                    }
                                    KeyCode::PageDown => {
                                        self.apply_scroll_to(Panel::AllLogs, SCROLL_PAGE_SIZE)
                                    }
                                    KeyCode::PageUp => {
                                        self.apply_scroll_to(Panel::AllLogs, -SCROLL_PAGE_SIZE)
                                    }
                                    _ => {}
                                },
                                Panel::SqlDetail => match key.code {
                                    KeyCode::Char('j') | KeyCode::Down => {
                                        self.apply_scroll_to(Panel::SqlDetail, 1)
                                    }
                                    KeyCode::Char('k') | KeyCode::Up => {
                                        self.apply_scroll_to(Panel::SqlDetail, -1)
                                    }
                                    KeyCode::PageDown => {
                                        self.apply_scroll_to(Panel::SqlDetail, SCROLL_PAGE_SIZE)
                                    }
                                    KeyCode::PageUp => {
                                        self.apply_scroll_to(Panel::SqlDetail, -SCROLL_PAGE_SIZE)
                                    }
                                    _ => {}
                                },
                            }
                        }
                    },
                    Event::Mouse(mouse_event) if !self.copy_mode_enabled => {
                        let layout_info = self.layout_info.clone();
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

    pub fn next_request(&mut self) {
        if self.request_ids.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.request_ids.len();
        self.detail_scroll_offset = 0;
    }

    pub fn previous_request(&mut self) {
        if self.request_ids.is_empty() {
            return;
        }
        self.selected_index = if self.selected_index == 0 {
            self.request_ids.len() - 1
        } else {
            self.selected_index - 1
        };
        self.detail_scroll_offset = 0;
    }

    fn apply_scroll_to(&mut self, panel: Panel, n: i8) {
        if n > 0 {
            match panel {
                Panel::Detail => {
                    let max_scroll = self.get_max_detail_scroll();
                    self.detail_scroll_offset =
                        (self.detail_scroll_offset + n as usize).min(max_scroll);
                }
                Panel::AllLogs => {
                    let max_scroll = self.get_max_stream_scroll();
                    self.all_scroll_offset = (self.all_scroll_offset + n as usize).min(max_scroll);
                }
                Panel::SqlDetail => {
                    self.sql_scroll_offset = (self.sql_scroll_offset + n as usize).min(100);
                }
                _ => {}
            }
        } else if n < 0 {
            match panel {
                Panel::Detail => {
                    self.detail_scroll_offset = self
                        .detail_scroll_offset
                        .saturating_sub(n.unsigned_abs() as usize)
                }
                Panel::AllLogs => {
                    self.all_scroll_offset = self
                        .all_scroll_offset
                        .saturating_sub(n.unsigned_abs() as usize)
                }
                Panel::SqlDetail => {
                    self.sql_scroll_offset = self
                        .sql_scroll_offset
                        .saturating_sub(n.unsigned_abs() as usize)
                }
                _ => {}
            }
        }
    }

    fn get_max_detail_scroll(&self) -> usize {
        if let Some(group) = self.selected_group() {
            let entry_count = group.entries.len();
            let viewport_height = self.layout_info.right_region.height.saturating_sub(4) as usize;
            entry_count.saturating_sub(viewport_height).max(0)
        } else {
            0
        }
    }

    fn get_max_stream_scroll(&self) -> usize {
        let log_count = self.all_logs.len();
        let viewport_height = self.layout_info.bottom_region.height.saturating_sub(3) as usize;
        log_count.saturating_sub(viewport_height).max(0)
    }

    pub fn get_visible_logs(&self, viewport_height: usize) -> Vec<&LogEntry> {
        let total_logs = self.all_logs.len();

        if total_logs == 0 {
            return Vec::new();
        }

        let start_idx = self.all_scroll_offset.min(total_logs.saturating_sub(1));
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

        if is_new_request {
            self.request_ids.insert(0, request_id.clone());
            self.first_timestamps
                .insert(request_id.clone(), log_entry.timestamp);

            if self.request_ids.len() == 1 {
                self.selected_index = 0;
            } else {
                self.selected_index += 1;
            }

            let mut new_group = LogGroup {
                title: "...".to_string(),
                entries: Vec::with_capacity(8),
                finished: false,
            };

            if message.contains("Started ") {
                if let Some(start_pos) = message.find("Started ") {
                    new_group.title = message[(start_pos + 8)..].to_string();
                } else {
                    new_group.title = message.to_string();
                }
            }

            if message.contains("Completed ") {
                new_group.finished = true;
            }

            new_group.entries.push(log_entry.clone());
            self.logs_by_request_id.insert(request_id, new_group);
        } else {
            let group = self.logs_by_request_id.get_mut(&request_id).unwrap();

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
        self.focused_panel = match self.focused_panel {
            Panel::List => Panel::Detail,
            Panel::Detail => Panel::AllLogs,
            Panel::AllLogs => Panel::List,
            Panel::SqlDetail => Panel::List,
        };
    }

    pub fn toggle_focus_reverse(&mut self) {
        self.focused_panel = match self.focused_panel {
            Panel::List => Panel::AllLogs,
            Panel::Detail => Panel::List,
            Panel::AllLogs => Panel::Detail,
            Panel::SqlDetail => Panel::AllLogs,
        };
    }

    pub fn jump_to_latest(&mut self) {
        if !self.request_ids.is_empty() {
            self.selected_index = 0;
            self.detail_scroll_offset = 0;
        }
    }

    pub fn toggle_all_logs_panel(&mut self) {
        self.focused_panel = match self.focused_panel {
            Panel::AllLogs => Panel::List,
            _ => Panel::AllLogs,
        };
    }

    fn handle_mouse_event(&mut self, mouse_event: event::MouseEvent, layout_info: &LayoutInfo) {
        let x = mouse_event.column;
        let y = mouse_event.row;

        match mouse_event.kind {
            event::MouseEventKind::ScrollDown | event::MouseEventKind::ScrollUp => {
                if is_in_region(x, y, &layout_info.left_region) {
                    return;
                }

                if is_in_region(x, y, &layout_info.right_region) {
                    if let event::MouseEventKind::ScrollDown = mouse_event.kind {
                        self.apply_scroll_to(Panel::Detail, 1);
                    } else {
                        self.apply_scroll_to(Panel::Detail, -1);
                    }
                    return;
                }

                if is_in_region(x, y, &layout_info.sql_region) {
                    if let event::MouseEventKind::ScrollDown = mouse_event.kind {
                        self.apply_scroll_to(Panel::SqlDetail, 1);
                    } else {
                        self.apply_scroll_to(Panel::SqlDetail, -1);
                    }
                    return;
                }

                if is_in_region(x, y, &layout_info.bottom_region) {
                    if let event::MouseEventKind::ScrollDown = mouse_event.kind {
                        self.apply_scroll_to(Panel::AllLogs, 1);
                    } else {
                        self.apply_scroll_to(Panel::AllLogs, -1);
                    }
                    return;
                }

                return;
            }
            _ => {}
        }

        if is_in_region(x, y, &layout_info.left_region) {
            if let event::MouseEventKind::Down(event::MouseButton::Left) = mouse_event.kind {
                self.focused_panel = Panel::List;

                let row_in_list = y.saturating_sub(layout_info.left_region.y + 2);
                let clicked_index = row_in_list as usize;

                if clicked_index < self.request_ids.len() {
                    self.selected_index = clicked_index;
                    self.detail_scroll_offset = 0;
                }
            }
            return;
        }

        if is_in_region(x, y, &layout_info.right_region) {
            if let event::MouseEventKind::Down(event::MouseButton::Left) = mouse_event.kind {
                self.focused_panel = Panel::Detail;
            }
            return;
        }

        if is_in_region(x, y, &layout_info.sql_region) {
            if let event::MouseEventKind::Down(event::MouseButton::Left) = mouse_event.kind {
                self.focused_panel = Panel::SqlDetail;
                self.debug_text = format!("SQL panel clicked");
            }
            return;
        }

        if is_in_region(x, y, &layout_info.bottom_region) {
            if let event::MouseEventKind::Down(event::MouseButton::Left) = mouse_event.kind {
                self.focused_panel = Panel::AllLogs;
            }
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
        let is_all_logs_focused = matches!(self.focused_panel, Panel::AllLogs);
        let max_stream_scroll = self.get_max_stream_scroll();
        let at_bottom = self.all_scroll_offset >= max_stream_scroll.saturating_sub(1);

        if is_all_logs_focused || at_bottom {
            self.all_scroll_offset = self.get_max_stream_scroll();
        }
    }
}

fn is_in_region(x: u16, y: u16, area: &Rect) -> bool {
    x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
}
