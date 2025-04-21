use ratatui::layout::Rect;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Panel {
    RequestList,
    RequestDetail,
    SqlInfo,
}

impl Panel {
    pub(crate) fn all() -> [Panel; 3] {
        [Panel::RequestList, Panel::RequestDetail, Panel::SqlInfo]
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
    pub fn region(&self, panel: Panel) -> Rect {
        self.regions.get(&panel).cloned().unwrap_or_default()
    }
}

pub fn calculate_layout(area: Rect) -> LayoutInfo {
    use ratatui::layout::{Constraint, Direction, Layout};

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(2, 10),
            Constraint::Ratio(6, 10),
            Constraint::Ratio(2, 10),
        ])
        .split(area);

    LayoutInfo::new()
        .with_region(Panel::RequestList, top_chunks[0])
        .with_region(Panel::RequestDetail, top_chunks[1])
        .with_region(Panel::SqlInfo, top_chunks[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn test_layout_info_with_region() {
        let rect = Rect::new(0, 0, 10, 10);
        let layout = LayoutInfo::new().with_region(Panel::RequestList, rect);

        assert_eq!(layout.regions.len(), 1);
        assert_eq!(layout.region(Panel::RequestList), rect);
    }

    #[test]
    fn test_calculate_layout() {
        let area = Rect::new(0, 0, 100, 100);
        let layout = calculate_layout(area);

        // Check all panels exist
        for panel in Panel::all().iter() {
            let region = layout.region(*panel);
            assert!(region.width > 0);
            assert!(region.height > 0);
        }

        // Check basic layout properties
        let request_list = layout.region(Panel::RequestList);
        let request_detail = layout.region(Panel::RequestDetail);
        let sql_info = layout.region(Panel::SqlInfo);

        // RequestList and RequestDetail should be at the top
        assert_eq!(request_list.y, 0);
        assert_eq!(request_detail.y, 0);

        assert!(sql_info.y > request_detail.y);

        // RequestList should be to the left of RequestDetail
        assert!(request_list.x < request_detail.x);
    }
}
