use crate::layout::{LayoutInfo, Panel};

#[derive(Debug, Clone, Copy)]
pub enum ScrollDirection {
    Up(usize),
    Down(usize),
}

pub struct AppView {
    pub focused_panel: Panel,
    pub scroll_offsets: std::collections::HashMap<Panel, usize>,
    pub layout_info: LayoutInfo,
    pub panel_ratios: [f64; 3],
    pub dragging_border: Option<usize>,
}

impl AppView {
    const VIEW_PADDING: u16 = 4;

    pub fn new() -> Self {
        let mut scroll_offsets = std::collections::HashMap::new();
        scroll_offsets.insert(Panel::RequestList, 0);
        scroll_offsets.insert(Panel::RequestDetail, 0);
        scroll_offsets.insert(Panel::SqlInfo, 0);

        Self {
            focused_panel: Panel::RequestList,
            scroll_offsets,
            layout_info: LayoutInfo::new(),
            panel_ratios: [0.20, 0.60, 0.20],
            dragging_border: None,
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

    pub fn apply_scroll(&mut self, panel: Panel, direction: ScrollDirection, max_scroll: usize) {
        let current = self.get_scroll_offset(panel);
        let new_offset = match direction {
            ScrollDirection::Down(amount) => (current + amount).min(max_scroll),
            ScrollDirection::Up(amount) => current.saturating_sub(amount),
        };
        self.set_scroll_offset(panel, new_offset);
    }

    pub fn viewport_height(&self, panel: Panel) -> usize {
        let region = self.layout_info.region(panel);
        region.height.saturating_sub(Self::VIEW_PADDING) as usize
    }

    pub fn viewport_width(&self, panel: Panel) -> usize {
        let region = self.layout_info.region(panel);
        region.width.saturating_sub(Self::VIEW_PADDING) as usize
    }

    pub fn adjust_scroll_for_index(&mut self, panel: Panel, index: usize) {
        let viewport_height = self.viewport_height(panel);
        let current_offset = self.get_scroll_offset(panel);

        if index < current_offset {
            self.set_scroll_offset(panel, index);
        } else if index >= current_offset + viewport_height {
            self.set_scroll_offset(panel, index.saturating_sub(viewport_height - 1));
        }
    }

    pub fn is_in_region(x: u16, y: u16, area: &ratatui::layout::Rect) -> bool {
        x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
    }

    pub fn panel_at_point(&self, x: u16, y: u16) -> Option<Panel> {
        Panel::all()
            .into_iter()
            .find(|&panel| Self::is_in_region(x, y, &self.layout_info.region(panel)))
    }

    pub fn border_at_point(&self, x: u16) -> Option<usize> {
        let list_region = self.layout_info.region(Panel::RequestList);
        let detail_region = self.layout_info.region(Panel::RequestDetail);

        let border0 = list_region.x + list_region.width;
        let border1 = detail_region.x + detail_region.width;

        if x.abs_diff(border0) <= 1 {
            Some(0)
        } else if x.abs_diff(border1) <= 1 {
            Some(1)
        } else {
            None
        }
    }

    pub fn apply_drag(&mut self, x: u16, total_width: u16) {
        const MIN_RATIO: f64 = 0.10;

        let Some(border_idx) = self.dragging_border else {
            return;
        };

        let ratio = x as f64 / total_width as f64;

        match border_idx {
            0 => {
                // Dragging border between List and Detail
                let new_list = ratio.clamp(MIN_RATIO, 1.0 - self.panel_ratios[2] - MIN_RATIO);
                let new_detail = 1.0 - new_list - self.panel_ratios[2];
                if new_detail >= MIN_RATIO {
                    self.panel_ratios[0] = new_list;
                    self.panel_ratios[1] = new_detail;
                }
            }
            1 => {
                // Dragging border between Detail and Sql
                let new_sql = (1.0 - ratio).clamp(MIN_RATIO, 1.0 - self.panel_ratios[0] - MIN_RATIO);
                let new_detail = 1.0 - self.panel_ratios[0] - new_sql;
                if new_detail >= MIN_RATIO {
                    self.panel_ratios[1] = new_detail;
                    self.panel_ratios[2] = new_sql;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn test_app_view_new() {
        let view = AppView::new();
        assert_eq!(view.focused_panel, Panel::RequestList);
        assert_eq!(view.get_scroll_offset(Panel::RequestList), 0);
        assert_eq!(view.get_scroll_offset(Panel::RequestDetail), 0);
        assert_eq!(view.get_scroll_offset(Panel::SqlInfo), 0);
    }

    #[test]
    fn test_set_scroll_offset() {
        let mut view = AppView::new();

        view.set_scroll_offset(Panel::RequestList, 5);
        assert_eq!(view.get_scroll_offset(Panel::RequestList), 5);

        view.set_scroll_offset(Panel::RequestDetail, 10);
        assert_eq!(view.get_scroll_offset(Panel::RequestDetail), 10);
    }

    #[test]
    fn test_is_in_region() {
        let rect = Rect::new(10, 10, 20, 15);

        // Inside region
        assert!(AppView::is_in_region(15, 15, &rect));
        assert!(AppView::is_in_region(10, 10, &rect)); // Top-left corner
        assert!(AppView::is_in_region(29, 24, &rect)); // Bottom-right corner

        // Outside region
        assert!(!AppView::is_in_region(9, 15, &rect)); // Left out
        assert!(!AppView::is_in_region(30, 15, &rect)); // Right out
        assert!(!AppView::is_in_region(15, 9, &rect)); // Top out
        assert!(!AppView::is_in_region(15, 25, &rect)); // Bottom out
    }
}
