use color_eyre::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use tracing::debug;

use std::{
    fs::File,
    io::{self, BufRead, BufReader, Stdin},
    sync::mpsc,
    thread,
    time::Duration,
};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

mod app;
mod layout;
mod log_parser;
mod sql_info;
mod ui;

use app::App;

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
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            Clear(ClearType::All),
            LeaveAlternateScreen,
            DisableMouseCapture,
        );
        let _ = self.terminal.show_cursor();

        unsafe {
            let _ = libc::kill(0, libc::SIGPIPE);
        }
    }
}

fn init_tracing_subscriber() {
    let file = File::create("tracing.log").unwrap();
    let file_layer = fmt::layer().with_writer(file);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".into()),
        )
        .with(file_layer)
        .init();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing_subscriber();

    let (tx, rx) = mpsc::channel::<String>();
    let _reader_thread = {
        thread::spawn(move || {
            let stdin = io::stdin();
            process_output(stdin, tx);
        })
    };

    let mut stdout = io::stdout();
    execute!(
        stdout,
        Clear(ClearType::All),
        EnterAlternateScreen,
        EnableMouseCapture
    )?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut guard = CleanGuard {
        terminal: &mut terminal,
    };

    enable_raw_mode()?;
    let mut app = App::new();
    let result = app.run(guard.terminal(), rx);
    if let Err(err) = result {
        debug!("Application error: {:?}", err);
        return Err(err.into());
    }
    Ok(())
}

fn process_output(input: Stdin, tx: mpsc::Sender<String>) {
    let mut reader = BufReader::with_capacity(16 * 1024, input);
    let mut buffer = String::with_capacity(512);
    let wait_time = Duration::from_millis(1000);

    loop {
        buffer.clear();
        match reader.read_line(&mut buffer) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let trimmed = buffer.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if tx.send(trimmed.to_string()).is_err() {
                    debug!("Failed to send message to channel");
                    break;
                }
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    thread::sleep(wait_time);
                    continue;
                }

                debug!("Read error: {}", e);
                break;
            }
        }
        thread::sleep(wait_time);
    }
}
