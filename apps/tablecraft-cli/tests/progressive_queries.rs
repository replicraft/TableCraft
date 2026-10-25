//! Explicit SQL streams publish completed TSV batches and preserve original inputs.
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

#[path = "progressive_queries/boundaries.rs"]
mod boundaries;
#[path = "progressive_queries/count.rs"]
mod count;
#[path = "progressive_queries/count_operands.rs"]
mod count_operands;
#[path = "progressive_queries/execution.rs"]
mod execution;
#[path = "progressive_queries/stored_calculations.rs"]
mod stored_calculations;
#[path = "progressive_queries/top.rs"]
mod top;
#[path = "progressive_queries/transport.rs"]
mod transport;
#[path = "progressive_queries/union.rs"]
mod union;

use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct FileCopy {
    path: PathBuf,
    original: Vec<u8>,
}

impl FileCopy {
    fn new(bytes: Vec<u8>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-query-stream-{}-{}.mdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, &bytes).unwrap();
        Self {
            path,
            original: bytes,
        }
    }

    fn query(&self, sql: &str, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", "--database"])
            .arg(&self.path)
            .arg(sql)
            .args(arguments)
            .output()
            .unwrap()
    }
}

impl Drop for FileCopy {
    fn drop(&mut self) {
        assert_eq!(fs::read(&self.path).unwrap(), self.original);
        fs::remove_file(&self.path).unwrap();
    }
}

fn success(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    output.stdout
}

fn failure(output: Output) -> Vec<u8> {
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.is_empty());
    output.stdout
}
