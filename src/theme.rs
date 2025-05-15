use ratatui::style::{Color, Modifier, Style};

pub trait ColorExt {
    fn ansi(&self) -> &'static str;
    fn style(&self) -> Style;
    fn style_with_modifier(&self, modifier: Modifier) -> Style;
}

impl ColorExt for Color {
    fn ansi(&self) -> &'static str {
        match self {
            Color::Black => "\x1b[30m",
            Color::Red => "\x1b[31m",
            Color::Green => "\x1b[32m",
            Color::Yellow => "\x1b[33m",
            Color::Blue => "\x1b[34m",
            Color::Magenta => "\x1b[35m",
            Color::Cyan => "\x1b[36m",
            Color::Gray => "\x1b[37m",
            Color::DarkGray => "\x1b[90m",
            Color::LightRed => "\x1b[91m",
            Color::LightGreen => "\x1b[92m",
            Color::LightYellow => "\x1b[93m",
            Color::LightBlue => "\x1b[94m",
            Color::LightMagenta => "\x1b[95m",
            Color::LightCyan => "\x1b[96m",
            Color::White => "\x1b[97m",
            _ => "\x1b[39m", // Default color
        }
    }

    fn style(&self) -> Style {
        Style::default().fg(*self)
    }

    fn style_with_modifier(&self, modifier: Modifier) -> Style {
        self.style().add_modifier(modifier)
    }
}

pub struct Theme {
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub default: Color,
    pub border: Color,
    pub active_border: Color,
}

pub const THEME: Theme = Theme {
    success: Color::Green,
    warning: Color::Magenta,
    error: Color::Red,
    default: Color::White,
    border: Color::DarkGray,
    active_border: Color::White,
};

pub const ANSI_RESET: &str = "\x1b[0m";
