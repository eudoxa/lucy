use std::io::{self, BufRead, BufReader, Stdin};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

pub struct Reader {
    _reader_thread: JoinHandle<()>,
}

impl Reader {
    pub fn new() -> (Self, Receiver<String>) {
        let (tx, rx) = mpsc::channel::<String>();

        let reader_thread = thread::spawn(move || {
            let stdin = io::stdin();
            process_input(stdin, tx);
        });

        (Self { _reader_thread: reader_thread }, rx)
    }
}

fn process_input(input: Stdin, tx: Sender<String>) {
    let mut reader = BufReader::with_capacity(32 * 1024, input);
    let mut buffer = String::with_capacity(1024);

    loop {
        buffer.clear();
        match reader.read_line(&mut buffer) {
            Ok(0) => break,
            Ok(_) => {
                if let Err(e) = tx.send(buffer.clone()) {
                    tracing::debug!("Failed to send message to channel: {}", e);
                    break;
                }
            }
            Err(e) => {
                tracing::debug!("Input reader error: {}", e);
                break;
            }
        }
    }

    tracing::debug!("Input reader thread terminated");
}
