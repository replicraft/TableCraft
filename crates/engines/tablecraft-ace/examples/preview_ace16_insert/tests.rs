use super::*;
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct Directory(pub(crate) PathBuf);

impl Directory {
    pub(crate) fn new() -> Self {
        let ordinal = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tablecraft-insert-preview-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub(crate) fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

pub(crate) fn budget() -> ResourceBudget {
    ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET)
}

#[test]
fn invalid_complete_input_is_rejected_before_source_open_or_output_creation() {
    let directory = Directory::new();
    let requests = directory.file("requests.tsv", b"int32\t1\tbinary\tzz\n");
    let output = directory.0.join("candidate.accdb");
    let args = Args::try_parse_from([
        "insert".as_ref(),
        directory.0.join("absent.accdb").as_os_str(),
        output.as_os_str(),
        "Items".as_ref(),
        requests.as_os_str(),
    ])
    .unwrap();
    let error = run(args, &mut budget()).unwrap_err();
    assert!(error.to_string().contains("invalid Binary hex"));
    assert!(!output.exists());
}
