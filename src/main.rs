mod app;
mod app_state;
mod app_view;
mod input;
mod layout;
mod log_parser;
mod panel_components;
mod setup;
mod simple_formatter;
mod sql_info;
mod theme;

use color_eyre::Result;

struct TerminalGuard<B: ratatui::backend::Backend> {
    terminal: ratatui::Terminal<B>,
}

impl<B: ratatui::backend::Backend> TerminalGuard<B> {
    /// Create a new terminal guard
    pub fn new(terminal: ratatui::Terminal<B>) -> Self {
        Self { terminal }
    }

    /// Get a mutable reference to the wrapped terminal
    pub fn terminal(&mut self) -> &mut ratatui::Terminal<B> {
        &mut self.terminal
    }
}

impl<B: ratatui::backend::Backend> Drop for TerminalGuard<B> {
    fn drop(&mut self) {
        if let Err(err) = setup::cleanup(&mut self.terminal) {
            tracing::error!("Failed to clean up terminal: {}", err);
        }
    }
}

fn main() -> Result<()> {
    setup::initialize()?;

    let (_input_reader, rx) = input::Reader::new();
    let terminal = setup::initialize_terminal()?;
    let mut guard = TerminalGuard::new(terminal);

    let mut app = app::App::new();
    app.run(guard.terminal(), rx)?;

    Ok(())
}
