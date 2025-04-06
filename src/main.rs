use std::{
    io::{self, BufRead, BufReader, Stdin},
    panic, thread,
    time::Duration,
};

mod app;
mod app_state;
mod app_view;
mod components;
mod layout;
mod log_parser;
mod sql_info;

struct CleanGuard<'a, B: ratatui::backend::Backend> {
    terminal: &'a mut ratatui::Terminal<B>,
}

impl<B: ratatui::backend::Backend> CleanGuard<'_, B> {
    fn terminal(&mut self) -> &mut ratatui::Terminal<B> {
        self.terminal
    }
}

impl<B: ratatui::backend::Backend> Drop for CleanGuard<'_, B> {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture,
        );
        let _ = self.terminal.show_cursor();

        unsafe {
            let _ = libc::kill(0, libc::SIGPIPE);
        }
    }
}

fn setup_tracing_subscriber() {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
    let file_layer = fmt::layer().with_writer(std::fs::File::create("tracing.log").unwrap());
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
            .unwrap_or("unknown location".to_string());

        let message = panic_info
            .payload()
            .downcast_ref::<&str>()
            .unwrap_or(&"unknown message");

        tracing::error!("panic: {}", message);
        tracing::error!("location: {}", location);
        tracing::error!("backtrace: {:?}", backtrace);
    }));
}

fn process_output(input: Stdin, tx: std::sync::mpsc::Sender<String>) {
    let mut reader = BufReader::with_capacity(16 * 1024, input);
    let mut buffer = String::with_capacity(512);
    let wait_time = Duration::from_millis(1);

    loop {
        buffer.clear();
        match reader.read_line(&mut buffer) {
            Ok(0) => break,
            Ok(_) => {
                if let Err(e) = tx.send(buffer.to_string()) {
                    tracing::debug!("Failed to send message to channel: {}", e);
                    break;
                }
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    thread::sleep(wait_time);
                    continue;
                }
                tracing::debug!("Read error: {}", e);
                break;
            }
        }
        thread::sleep(wait_time);
    }
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    setup_tracing_subscriber();
    setup_panic_handler();

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let _reader_thread = {
        thread::spawn(move || {
            let stdin = io::stdin();
            process_output(stdin, tx);
        })
    };

    let mut stdout = io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    crossterm::terminal::enable_raw_mode()?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;
    let mut guard = CleanGuard {
        terminal: &mut terminal,
    };

    let mut app = app::App::new();
    app.run(guard.terminal(), rx)?;
    Ok(())
}
