mod protocol;
mod runner;

use protocol::{send, Command, Response};
use std::io::BufRead;

fn main() {
    let stdin = std::io::stdin();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(_) => break,
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cmd: Command = match serde_json::from_str(trimmed) {
            Ok(c) => c,
            Err(e) => {
                send(&Response::Error {
                    error: format!("parse error: {e}"),
                });
                continue;
            }
        };
        match cmd {
            Command::Run(run) => match runner::run_command(&run, &mut reader) {
                Ok(code) => send(&Response::Exited { exited: code }),
                Err(e) => send(&Response::Error { error: e }),
            },
            Command::Shutdown => break,
            Command::Cancel(_) => {
                // Cancel is only meaningful during run_command's poll loop.
            }
        }
    }
}
