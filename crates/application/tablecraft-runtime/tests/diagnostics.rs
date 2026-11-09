use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tablecraft_runtime::{Command, ContentsTarget, Error, Worker};

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
    // Keep this scoped-subscriber observation in its own test executable.
    // tracing-core's single-dispatch registration fast path can otherwise let
    // parallel quiet unit workers cache these shared callsites as disabled.
    let worker = tracing::subscriber::with_default(subscriber, || Worker::new(|| {}));
    let directory = tempfile::tempdir().unwrap();

    let commands = [
        Command::Open {
            path: directory.path().join("PRIVATE_PATH.accdb"),
            password: "PRIVATE_PASSWORD".into(),
        },
        Command::Page {
            table: "PRIVATE_TABLE".into(),
            offset: 0,
        },
        Command::Query {
            options: Default::default(),
            sql: "SELECT 'PRIVATE_SQL'".into(),
        },
        Command::Describe {
            table: "PRIVATE_SCHEMA_TABLE".into(),
        },
        Command::InspectCollection {
            target: ContentsTarget {
                document_revision: 0,
                table: "PRIVATE_CONTENTS_TABLE".into(),
                column: "PRIVATE_CONTENTS_COLUMN".into(),
                parent_ordinal: 0,
            },
        },
    ];
    for command in commands {
        let generation = worker.request(command).expect("fresh test worker identity");
        let deadline = Instant::now() + Duration::from_secs(10);
        let (actual, response) = loop {
            if let Some(reply) = worker.poll() {
                break reply;
            }
            assert!(Instant::now() < deadline, "worker did not finish");
            std::thread::yield_now();
        };
        assert_eq!(actual, generation);
        assert!(matches!(
            response,
            Err(Error::Io(_) | Error::NoDatabase | Error::StaleDocument)
        ));
    }

    let output = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert_eq!(output.matches("application read task completed").count(), 5);
    for operation in ["open", "page", "query", "describe", "inspect_collection"] {
        assert!(
            output.contains(&format!("operation=\"{operation}\"")),
            "{output}"
        );
    }
    assert!(output.contains("success=false"));
    assert!(output.contains("superseded=false"));
    assert!(!output.contains("PRIVATE_"));
}
