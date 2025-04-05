use crate::app_state::{AppState, LogEntry};
use crate::app_view::AppView;
use crate::components;
use crate::layout::{LayoutInfo, Panel};
use crossterm::event::{self, Event, KeyCode};

pub struct App {
    pub state: AppState,
    pub app_view: AppView,
    pub copy_mode_enabled: bool,
}

static SCROLL_PAGE_SIZE: i8 = 5;

impl App {
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
            app_view: AppView::new(),
            copy_mode_enabled: false,
        }
    }

    pub fn render(&mut self, f: &mut ratatui::Frame) {
        self.app_view.layout_info = crate::layout::calculate_layout(f.area());
        self.adjust_all_scroll_positions();

        let request_list_region = self.app_view.layout_info.region(Panel::RequestList);
        let request_detail_region = self.app_view.layout_info.region(Panel::RequestDetail);
        let log_stream_region = self.app_view.layout_info.region(Panel::LogStream);
        let sql_info_region = self.app_view.layout_info.region(Panel::SqlInfo);

        let request_list = components::build_list_component(self);
        f.render_widget(request_list, request_list_region);

        let detail_panel = components::build_detail_component(self);
        f.render_widget(detail_panel, request_detail_region);

        let log_stream = components::build_log_stream_component(self);
        f.render_widget(log_stream, log_stream_region);

        let sql_panel = components::build_sql_component(self);
        f.render_widget(sql_panel, sql_info_region);
    }

    pub fn run<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
        rx: std::sync::mpsc::Receiver<String>,
    ) -> color_eyre::Result<()> {
        let mut buffer_size: u8 = 10;

        loop {
            terminal.draw(|f| {
                self.render(f);
            })?;

            while let Ok(line) = rx.try_recv() {
                crate::log_parser::parse(&line).map(|entry| self.add_log_entry(entry));
                if buffer_size == 0 {
                    buffer_size = 10;
                    break;
                }
                buffer_size -= 1;
            }

            match crossterm::event::poll(std::time::Duration::from_millis(16)) {
                Ok(true) => {
                    let event_result = event::read();
                    if let Err(e) = &event_result {
                        tracing::debug!("Event read error: {:?}", e);
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
                            KeyCode::Char('m') | KeyCode::Char('M') => self.toggle_copy_mode()?,
                            KeyCode::Char('d')
                                if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                            {
                                match self.app_view.focused_panel {
                                    Panel::RequestList => {
                                        self.next_request(3);
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
                                        self.previous_request(3);
                                    }
                                    Panel::RequestDetail => self
                                        .apply_scroll_to(Panel::RequestDetail, -SCROLL_PAGE_SIZE),
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
                                    KeyCode::Char('j') | KeyCode::Down => self.next_request(1),
                                    KeyCode::Char('k') | KeyCode::Up => self.previous_request(1),
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
                Ok(false) => {}
                Err(e) => {
                    tracing::debug!("Poll error: {:?}", e);
                    continue;
                }
            }
        }
    }

    fn select_request(&mut self, index: usize) {
        if self.state.select_request(index) {
            self.app_view.set_scroll_offset(Panel::RequestDetail, 0);
            self.app_view
                .adjust_scroll_for_index(Panel::RequestList, self.state.selected_index);
        }
    }

    pub fn next_request(&mut self, n: usize) {
        if self.state.next_request(n) {
            self.app_view.set_scroll_offset(Panel::RequestDetail, 0);
            self.app_view
                .adjust_scroll_for_index(Panel::RequestList, self.state.selected_index);
        }
    }

    pub fn previous_request(&mut self, n: usize) {
        if self.state.previous_request(n) {
            self.app_view.set_scroll_offset(Panel::RequestDetail, 0);
            self.app_view
                .adjust_scroll_for_index(Panel::RequestList, self.state.selected_index);
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
        self.state.selected_entries_count().saturating_sub(1)
    }

    fn get_max_sql_scroll(&self) -> usize {
        self.state
            .selected_sql_line_count()
            .saturating_sub(self.app_view.get_viewport_height(Panel::SqlInfo))
            .max(0)
    }

    fn get_max_stream_scroll(&self) -> usize {
        let log_count = self.state.logs_count();
        let viewport_height = self.app_view.get_viewport_height(Panel::LogStream);
        log_count.saturating_sub(viewport_height)
    }

    pub fn get_visible_logs(&self, viewport_height: usize) -> Vec<&LogEntry> {
        let start_idx = self.app_view.get_scroll_offset(Panel::LogStream);
        self.state.visible_logs(start_idx, viewport_height)
    }

    pub fn add_log_entry(&mut self, log_entry: LogEntry) {
        let is_new_request = self.state.add_log_entry(log_entry);

        if is_new_request {
            if matches!(self.app_view.focused_panel, Panel::RequestList) {
                self.app_view
                    .adjust_scroll_for_index(Panel::RequestList, self.state.selected_index);
            }
        }

        self.auto_scroll_if_needed();
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
        self.select_request(0);
    }

    fn handle_mouse_event(&mut self, mouse_event: event::MouseEvent, layout_info: &LayoutInfo) {
        let (x, y) = (mouse_event.column, mouse_event.row);

        match mouse_event.kind {
            event::MouseEventKind::ScrollDown | event::MouseEventKind::ScrollUp => {
                match self.app_view.get_panel_at_point(x, y) {
                    Some(Panel::RequestList) => match mouse_event.kind {
                        event::MouseEventKind::ScrollDown => self.next_request(1),
                        event::MouseEventKind::ScrollUp => self.previous_request(1),
                        _ => {}
                    },
                    Some(panel) => match mouse_event.kind {
                        event::MouseEventKind::ScrollDown => self.apply_scroll_to(panel, 1),
                        event::MouseEventKind::ScrollUp => self.apply_scroll_to(panel, -1),
                        _ => {}
                    },
                    None => {}
                }
            }

            event::MouseEventKind::Down(event::MouseButton::Left) => {
                match self.app_view.get_panel_at_point(x, y) {
                    Some(panel) if matches!(panel, Panel::RequestList) => {
                        self.app_view.focused_panel = panel;
                        let row_in_list =
                            y.saturating_sub(layout_info.region(Panel::RequestList).y + 2);
                        let current_offset = self.app_view.get_scroll_offset(Panel::RequestList);
                        let clicked_index = current_offset + row_in_list as usize;

                        if clicked_index < self.state.request_ids.len() {
                            self.select_request(clicked_index);
                        }
                    }
                    Some(panel) => {
                        self.app_view.focused_panel = panel;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    pub fn toggle_copy_mode(&mut self) -> color_eyre::Result<()> {
        self.copy_mode_enabled = !self.copy_mode_enabled;

        let mut stdout = std::io::stdout();
        if self.copy_mode_enabled {
            crossterm::execute!(stdout, crossterm::event::DisableMouseCapture)?;
        } else {
            crossterm::execute!(stdout, crossterm::event::EnableMouseCapture)?;
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

    fn adjust_all_scroll_positions(&mut self) {
        self.adjust_request_list_scroll();

        self.auto_scroll_if_needed();
    }

    fn adjust_request_list_scroll(&mut self) {
        let viewport_height = self.app_view.get_viewport_height(Panel::RequestList);

        if self.state.request_ids.len() > viewport_height {
            let current_offset = self.app_view.get_scroll_offset(Panel::RequestList);

            let new_offset = if self.state.selected_index < current_offset {
                self.state.selected_index
            } else if self.state.selected_index >= current_offset + viewport_height {
                self.state
                    .selected_index
                    .saturating_sub(viewport_height - 1)
            } else {
                current_offset
            };

            self.app_view
                .set_scroll_offset(Panel::RequestList, new_offset);
        } else {
            self.app_view.set_scroll_offset(Panel::RequestList, 0);
        }
    }
}
