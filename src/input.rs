use std::io::{self, BufRead, BufReader, Stdin};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct Reader {
    #[allow(dead_code)]
    reader_thread: JoinHandle<()>,
}

impl Reader {
    pub fn new() -> (Self, Receiver<String>) {
        let (tx, rx) = mpsc::channel::<String>();

        let reader_thread = thread::spawn(move || {
            let stdin = io::stdin();
            process_input(stdin, tx);
        });

        (Self { reader_thread }, rx)
    }
}

fn process_input(input: Stdin, tx: Sender<String>) {
    let mut reader = BufReader::with_capacity(16 * 1024, input);
    let mut buffer = String::with_capacity(512);
    let wait_time = Duration::from_millis(1);

    loop {
        buffer.clear();
        match reader.read_line(&mut buffer) {
            Ok(0) => {
                break;
            }
            Ok(_) => {
                if let Err(e) = tx.send(buffer.clone()) {
                    tracing::debug!("Failed to send message to channel: {}", e);
                    break;
                }
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    thread::sleep(wait_time);
                    continue;
                }
                tracing::debug!("Input reader error: {}", e);
                break;
            }
        }
        thread::sleep(wait_time);
    }

    tracing::debug!("Input reader thread terminated");
}
