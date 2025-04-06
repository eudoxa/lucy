use color_eyre::Result;
use std::io;
use std::panic;

pub fn initialize() -> Result<()> {
    color_eyre::install()?;
    setup_tracing_subscriber();
    setup_panic_handler();
    Ok(())
}

pub fn cleanup<B>(terminal: &mut ratatui::Terminal<B>) -> Result<()>
where
    B: ratatui::backend::Backend,
{
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    unsafe {
        libc::kill(0, libc::SIGPIPE);
    }
    Ok(())
}

fn setup_tracing_subscriber() {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

    let file_layer = match std::fs::File::create("tracing.log") {
        Ok(file) => fmt::layer().with_writer(file),
        Err(err) => {
            panic!("Failed to create tracing log file: {}", err);
        }
    };

    let default = if std::env::var("LUCY_DEV").is_ok() {
        "debug"
    } else {
        "info"
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| default.into()),
        )
        .with(file_layer)
        .init();
}

fn setup_panic_handler() {
    panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        let location = panic_info
            .location()
            .map(|location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            })
            .unwrap_or_else(|| "unknown location".to_string());

        let message = match panic_info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => match panic_info.payload().downcast_ref::<String>() {
                Some(s) => s.as_str(),
                None => "unknown panic message",
            },
        };

        tracing::error!("panic: {}", message);
        tracing::error!("location: {}", location);
        tracing::error!("backtrace: {:?}", backtrace);
    }));
}

pub fn initialize_terminal()
-> Result<ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>> {
    let mut stdout = io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    crossterm::terminal::enable_raw_mode()?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let terminal = ratatui::Terminal::new(backend)?;
    Ok(terminal)
}
