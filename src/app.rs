use crate::app_state::{AppState, LogEntry};
use crate::app_view::{AppView, ScrollDirection};
use crate::layout::{LayoutInfo, Panel};
use crate::panel_components;
use crossterm::event::{self, Event, KeyCode};

const SCROLL_UNIT: usize = 1;
const SCROLL_PAGE_SIZE: usize = 10;
const REQUEST_SKIP_COUNT: usize = 3;

pub enum SearchTarget {
    RequestList,
    DetailLog,
}

pub struct App {
    pub state: AppState,
    pub app_view: AppView,
    pub copy_mode_enabled: bool,
    pub simple_mode_enabled: bool,
    pub search_mode: Option<SearchTarget>,
    pub search_query: String,
    pub filtered_indices: Option<Vec<usize>>,
    pub detail_search_query: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
            app_view: AppView::new(),
            copy_mode_enabled: false,
            simple_mode_enabled: false,
            search_mode: None,
            search_query: String::new(),
            filtered_indices: None,
            detail_search_query: String::new(),
        }
    }

    pub fn render(&mut self, f: &mut ratatui::Frame) {
        if self.copy_mode_enabled {
            let focused = self.app_view.focused_panel;
            self.app_view.layout_info =
                crate::layout::calculate_single_panel_layout(f.area(), focused);
            let region = self.app_view.layout_info.region(focused);
            match focused {
                Panel::RequestList => {
                    let widget = panel_components::build_list_component(self);
                    f.render_widget(widget, region);
                }
                Panel::RequestDetail => {
                    let widget = panel_components::build_detail_component(self);
                    f.render_widget(widget, region);
                }
                Panel::SqlInfo => {
                    let widget = panel_components::build_sql_component(self);
                    f.render_widget(widget, region);
                }
            }
        } else {
            self.app_view.layout_info =
                crate::layout::calculate_layout(f.area(), &self.app_view.panel_ratios);

            let request_list_region = self.app_view.layout_info.region(Panel::RequestList);
            let request_detail_region = self.app_view.layout_info.region(Panel::RequestDetail);
            let sql_info_region = self.app_view.layout_info.region(Panel::SqlInfo);

            let request_list = panel_components::build_list_component(self);
            f.render_widget(request_list, request_list_region);

            let detail_panel = panel_components::build_detail_component(self);
            f.render_widget(detail_panel, request_detail_region);

            let sql_panel = panel_components::build_sql_component(self);
            f.render_widget(sql_panel, sql_info_region);
        }
    }

    pub fn run<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
        rx: std::sync::mpsc::Receiver<String>,
    ) -> color_eyre::Result<()> {
        let mut batch_size: u8 = 10;

        loop {
            terminal.draw(|f| {
                self.render(f);
            })?;

            while let Ok(line) = rx.try_recv() {
                if let Some(entry) = crate::log_parser::parse(&line) {
                    self.add_log_entry(entry);
                }

                if batch_size == 0 {
                    batch_size = 10;
                    break;
                }
                batch_size -= 1;
            }

            match crossterm::event::poll(std::time::Duration::from_millis(16)) {
                Ok(true) => {
                    let event_result = event::read();
                    if let Err(e) = &event_result {
                        tracing::debug!("Event read error: {:?}", e);
                        continue;
                    }

                    match event_result.unwrap() {
                        Event::Key(key) if self.search_mode.is_some() => match key.code {
                            KeyCode::Esc => {
                                match &self.search_mode {
                                    Some(SearchTarget::RequestList) => {
                                        self.search_query.clear();
                                        self.filtered_indices = None;
                                    }
                                    Some(SearchTarget::DetailLog) => {
                                        self.detail_search_query.clear();
                                    }
                                    None => {}
                                }
                                self.search_mode = None;
                            }
                            KeyCode::Enter => {
                                self.search_mode = None;
                                // Keep filter/highlight active
                            }
                            KeyCode::Backspace => match &self.search_mode {
                                Some(SearchTarget::RequestList) => {
                                    self.search_query.pop();
                                    self.update_filter();
                                }
                                Some(SearchTarget::DetailLog) => {
                                    self.detail_search_query.pop();
                                }
                                None => {}
                            },
                            KeyCode::Char('c')
                                if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                            {
                                return Ok(());
                            }
                            KeyCode::Char(c) => match &self.search_mode {
                                Some(SearchTarget::RequestList) => {
                                    self.search_query.push(c);
                                    self.update_filter();
                                }
                                Some(SearchTarget::DetailLog) => {
                                    self.detail_search_query.push(c);
                                }
                                None => {}
                            },
                            _ => {}
                        },
                        Event::Key(key) => match key.code {
                            KeyCode::Char('c')
                                if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                            {
                                return Ok(());
                            }
                            KeyCode::Char('/') => match self.app_view.focused_panel {
                                Panel::RequestList => {
                                    self.search_mode = Some(SearchTarget::RequestList);
                                    self.search_query.clear();
                                    self.filtered_indices = None;
                                }
                                Panel::RequestDetail => {
                                    self.search_mode = Some(SearchTarget::DetailLog);
                                    self.detail_search_query.clear();
                                }
                                _ => {}
                            },
                            KeyCode::Esc
                                if self.filtered_indices.is_some()
                                    || !self.detail_search_query.is_empty() =>
                            {
                                self.search_query.clear();
                                self.filtered_indices = None;
                                self.detail_search_query.clear();
                            }
                            KeyCode::BackTab => self.toggle_focus_reverse(),
                            KeyCode::Tab => self.toggle_focus(),
                            KeyCode::Char(' ') => self.jump_to_latest(),
                            KeyCode::Char('m') | KeyCode::Char('M') => self.toggle_copy_mode()?,
                            KeyCode::Char('s') | KeyCode::Char('S') => self.toggle_simple_mode()?,
                            KeyCode::Char('d')
                                if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                            {
                                match self.app_view.focused_panel {
                                    Panel::RequestList => {
                                        self.next_request(REQUEST_SKIP_COUNT);
                                    }
                                    Panel::RequestDetail => self.apply_scroll_to(
                                        Panel::RequestDetail,
                                        SCROLL_PAGE_SIZE as i8,
                                    ),
                                    Panel::SqlInfo => {
                                        self.apply_scroll_to(Panel::SqlInfo, SCROLL_PAGE_SIZE as i8)
                                    }
                                }
                            }
                            KeyCode::Char('u')
                                if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                            {
                                match self.app_view.focused_panel {
                                    Panel::RequestList => {
                                        self.previous_request(REQUEST_SKIP_COUNT);
                                    }
                                    Panel::RequestDetail => self.apply_scroll_to(
                                        Panel::RequestDetail,
                                        -(SCROLL_PAGE_SIZE as i8),
                                    ),
                                    Panel::SqlInfo => self
                                        .apply_scroll_to(Panel::SqlInfo, -(SCROLL_PAGE_SIZE as i8)),
                                }
                            }
                            _ => match self.app_view.focused_panel {
                                Panel::RequestList => match key.code {
                                    KeyCode::Char('j') | KeyCode::Down => {
                                        self.next_request(SCROLL_UNIT)
                                    }
                                    KeyCode::Char('k') | KeyCode::Up => {
                                        self.previous_request(SCROLL_UNIT)
                                    }
                                    _ => {}
                                },
                                _ => match key.code {
                                    KeyCode::Char('j') | KeyCode::Down => self.apply_scroll_to(
                                        self.app_view.focused_panel,
                                        SCROLL_UNIT as i8,
                                    ),
                                    KeyCode::Char('k') | KeyCode::Up => self.apply_scroll_to(
                                        self.app_view.focused_panel,
                                        -(SCROLL_UNIT as i8),
                                    ),
                                    KeyCode::PageDown => self.apply_scroll_to(
                                        self.app_view.focused_panel,
                                        SCROLL_PAGE_SIZE as i8,
                                    ),
                                    KeyCode::PageUp => self.apply_scroll_to(
                                        self.app_view.focused_panel,
                                        -(SCROLL_PAGE_SIZE as i8),
                                    ),
                                    _ => {}
                                },
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

    fn apply_scroll_to(&mut self, panel: Panel, amount: i8) {
        let max_scroll = match panel {
            Panel::RequestDetail => self.get_max_detail_scroll(),
            Panel::SqlInfo => self.get_max_sql_scroll(),
            _ => 0,
        };

        let direction = if amount < 0 {
            ScrollDirection::Up(amount.unsigned_abs() as usize)
        } else {
            ScrollDirection::Down(amount as usize)
        };

        self.app_view.apply_scroll(panel, direction, max_scroll);
    }

    fn get_max_request_list_scroll(&self) -> usize {
        self.state
            .request_ids
            .len()
            .saturating_sub(self.app_view.viewport_height(Panel::RequestList))
    }

    fn get_max_detail_scroll(&self) -> usize {
        self.state.selected_entries_count().saturating_sub(1)
    }

    fn get_max_sql_scroll(&self) -> usize {
        self.state
            .selected_sql_line_count()
            .saturating_sub(self.app_view.viewport_height(Panel::SqlInfo))
            .max(0)
    }

    pub fn add_log_entry(&mut self, log_entry: LogEntry) {
        let is_new_request = self.state.add_log_entry(log_entry);
        if is_new_request {
            self.app_view
                .adjust_scroll_for_index(Panel::RequestList, self.state.selected_index);
        }
    }

    pub fn toggle_focus(&mut self) {
        self.app_view.focused_panel = match self.app_view.focused_panel {
            Panel::RequestList => Panel::RequestDetail,
            Panel::RequestDetail => Panel::SqlInfo,
            Panel::SqlInfo => Panel::RequestList,
        };
    }

    pub fn toggle_focus_reverse(&mut self) {
        self.app_view.focused_panel = match self.app_view.focused_panel {
            Panel::RequestList => Panel::SqlInfo,
            Panel::RequestDetail => Panel::RequestList,
            Panel::SqlInfo => Panel::RequestDetail,
        };
    }

    pub fn jump_to_latest(&mut self) {
        self.select_request(0);
    }

    fn toggle_simple_mode(&mut self) -> color_eyre::Result<()> {
        self.simple_mode_enabled = !self.simple_mode_enabled;
        Ok(())
    }

    fn update_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_indices = None;
            return;
        }
        let query_lower = self.search_query.to_lowercase();
        let indices: Vec<usize> = self
            .state
            .request_ids
            .iter()
            .enumerate()
            .filter(|(_, req_id)| {
                self.state
                    .logs_by_request_id
                    .get(*req_id)
                    .is_some_and(|group| group.title.to_lowercase().contains(&query_lower))
            })
            .map(|(i, _)| i)
            .collect();
        self.filtered_indices = Some(indices);
    }

    pub fn visible_request_ids(&self) -> Vec<(usize, &str)> {
        match &self.filtered_indices {
            Some(indices) => indices
                .iter()
                .filter_map(|&i| {
                    self.state
                        .request_ids
                        .get(i)
                        .map(|id| (i, id.as_str()))
                })
                .collect(),
            None => self
                .state
                .request_ids
                .iter()
                .enumerate()
                .map(|(i, id)| (i, id.as_str()))
                .collect(),
        }
    }

    fn handle_mouse_event(&mut self, mouse_event: event::MouseEvent, layout_info: &LayoutInfo) {
        let (x, y) = (mouse_event.column, mouse_event.row);

        match mouse_event.kind {
            event::MouseEventKind::ScrollDown | event::MouseEventKind::ScrollUp => {
                match self.app_view.panel_at_point(x, y) {
                    Some(Panel::RequestList) => match mouse_event.kind {
                        event::MouseEventKind::ScrollDown => self.app_view.apply_scroll(
                            Panel::RequestList,
                            ScrollDirection::Down(SCROLL_UNIT),
                            self.get_max_request_list_scroll(),
                        ),
                        event::MouseEventKind::ScrollUp => self.app_view.apply_scroll(
                            Panel::RequestList,
                            ScrollDirection::Up(SCROLL_UNIT),
                            self.get_max_request_list_scroll(),
                        ),

                        _ => {}
                    },
                    Some(panel) => match mouse_event.kind {
                        event::MouseEventKind::ScrollDown => {
                            self.apply_scroll_to(panel, SCROLL_UNIT as i8)
                        }
                        event::MouseEventKind::ScrollUp => {
                            self.apply_scroll_to(panel, -(SCROLL_UNIT as i8))
                        }
                        _ => {}
                    },
                    None => {}
                }
            }

            event::MouseEventKind::Down(event::MouseButton::Left) => {
                if let Some(border_idx) = self.app_view.border_at_point(x) {
                    self.app_view.dragging_border = Some(border_idx);
                } else {
                    match self.app_view.panel_at_point(x, y) {
                        Some(panel) if matches!(panel, Panel::RequestList) => {
                            self.app_view.focused_panel = panel;
                            let row_in_list =
                                y.saturating_sub(layout_info.region(Panel::RequestList).y + 2);
                            let current_offset =
                                self.app_view.get_scroll_offset(Panel::RequestList);
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
            }

            event::MouseEventKind::Drag(event::MouseButton::Left) => {
                if self.app_view.dragging_border.is_some() {
                    let total_width = layout_info.region(Panel::RequestList).width
                        + layout_info.region(Panel::RequestDetail).width
                        + layout_info.region(Panel::SqlInfo).width;
                    self.app_view.apply_drag(x, total_width);
                }
            }

            event::MouseEventKind::Up(event::MouseButton::Left) => {
                self.app_view.dragging_border = None;
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
}
