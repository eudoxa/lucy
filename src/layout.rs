use ratatui::layout::Rect;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Panel {
    RequestList,
    RequestDetail,
    LogStream,
    SqlInfo,
}

impl Panel {
    pub(crate) fn all() -> [Panel; 4] {
        [
            Panel::RequestList,
            Panel::RequestDetail,
            Panel::LogStream,
            Panel::SqlInfo,
        ]
    }
}

#[derive(Default, Debug, Clone)]
pub struct LayoutInfo {
    regions: HashMap<Panel, Rect>,
}

impl LayoutInfo {
    pub fn new() -> Self {
        Self {
            regions: HashMap::new(),
        }
    }

    pub fn with_region(mut self, panel: Panel, rect: Rect) -> Self {
        self.regions.insert(panel, rect);
        self
    }

    pub fn get_region(&self, panel: Panel) -> Rect {
        *self.regions.get(&panel).unwrap_or(&Rect::default())
    }

    pub fn region(&self, panel: Panel) -> Rect {
        self.get_region(panel)
    }
}

pub fn calculate_layout(area: Rect) -> LayoutInfo {
    use ratatui::layout::{Constraint, Direction, Layout};

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(4, 10), Constraint::Ratio(6, 10)])
        .split(main_chunks[0]);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(8, 10), Constraint::Ratio(2, 10)])
        .split(main_chunks[1]);

    LayoutInfo::new()
        .with_region(Panel::RequestList, top_chunks[0])
        .with_region(Panel::RequestDetail, top_chunks[1])
        .with_region(Panel::LogStream, bottom_chunks[0])
        .with_region(Panel::SqlInfo, bottom_chunks[1])
}
