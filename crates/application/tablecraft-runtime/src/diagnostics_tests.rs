use super::{Command, Error, Worker};
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn worker_inherits_subscriber_without_recording_command_payloads() {
    let capture = Capture::default();
    let writer = capture.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .without_time()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let worker = tracing::subscriber::with_default(subscriber, || {
        Worker::test_worker(|_, _| Err(Error::NoDatabase))
    });

    let commands = [
        Command::Open {
            path: "PRIVATE_PATH.accdb".into(),
            password: "PRIVATE_PASSWORD".into(),
        },
        Command::Page {
            table: "PRIVATE_TABLE".into(),
            offset: 0,
        },
        Command::Query {
            sql: "SELECT 'PRIVATE_SQL'".into(),
        },
    ];
    for command in commands {
        let generation = worker.request(command);
        let (actual, response) = super::tests::poll(&worker);
        assert_eq!(actual, generation);
        assert!(matches!(response, Err(Error::NoDatabase)));
    }

    let output = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert_eq!(output.matches("application read task completed").count(), 3);
    for operation in ["open", "page", "query"] {
        assert!(
            output.contains(&format!("operation=\"{operation}\"")),
            "{output}"
        );
    }
    assert!(output.contains("success=false"));
    assert!(output.contains("superseded=false"));
    assert!(!output.contains("PRIVATE_"));
}
