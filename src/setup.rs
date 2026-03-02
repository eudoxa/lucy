use color_eyre::Result;
use std::io;
use std::panic;

pub fn initialize() -> Result<()> {
    color_eyre::install()?;
    setup_tracing_subscriber()?;
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
    // Send SIGPIPE to the process group to signal the upstream pipe source
    // (e.g., `tail -f | lucy`) that we're done reading.
    // NOTE: kill(0, ...) sends to the entire process group, which will also
    // terminate any other processes in the same group. This is acceptable
    // because lucy is typically the last command in a pipe chain.
    unsafe {
        libc::kill(0, libc::SIGPIPE);
    }
    Ok(())
}

fn setup_tracing_subscriber() -> Result<()> {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

    let is_dev = std::env::var("LUCY_DEV").is_ok();

    if is_dev {
        let file = std::fs::File::create("tracing.log")?;
        let file_layer = fmt::layer().with_writer(file);

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "debug".into()),
            )
            .with(file_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "warn".into()),
            )
            .with(fmt::layer().with_writer(std::io::sink))
            .init();
    }

    Ok(())
}

fn setup_panic_handler() {
    panic::set_hook(Box::new(|panic_info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
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
