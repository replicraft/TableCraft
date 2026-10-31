#![cfg(all(
    feature = "filesystem-journal",
    any(target_os = "macos", target_os = "linux")
))]

use std::{
    fs::{self, OpenOptions},
    io::{Cursor, Read, Seek, SeekFrom, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::{
    IoError,
    journal::filesystem::{OfflineJournalPair, OfflineTarget},
    journal::{DurableStorage, RecoveryOutcome, V1State, inspect_v1, prepare_expected},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(10_000_000, 0, 10_000_000)
}

struct Memory(Cursor<Vec<u8>>);

impl Read for Memory {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(bytes)
    }
}

impl Write for Memory {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        panic!("source inspection must not flush")
    }
}

impl Seek for Memory {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(position)
    }
}

impl DurableStorage for Memory {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.0.get_ref().len() as u64)
    }
    fn sync_storage(&mut self) -> Result<(), IoError> {
        panic!("source inspection must not sync")
    }
}

struct RetiredSource {
    root: tempfile::TempDir,
    target: PathBuf,
    retired: PathBuf,
    committed: bool,
}

fn retired_source(committed: bool) -> RetiredSource {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    let active = root.path().join("active");
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&target)
        .unwrap();
    file.write_all(b"original").unwrap();
    drop(file);
    let owner = OfflineTarget::open(&target, &mut budget(), &|| false).unwrap();
    let mut pair = OfflineJournalPair::create(
        owner,
        &active,
        b"fixed-source-intent".to_vec(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let binding = fs::read(active.join("binding")).unwrap();
    let nonce: String = binding[16..32]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let retired = root.path().join(format!(".tablecraft-retired-{nonce}"));
    let outcome = {
        let (mut target, mut journal) = pair.storage().unwrap();
        let prepared = prepare_expected(
            &mut target,
            &mut journal,
            b"original",
            b"modified",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
            RecoveryOutcome::Committed
        } else {
            drop(prepared);
            let outcome = inspect_v1(&mut journal, &mut budget(), &|| false)
                .unwrap()
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(outcome, RecoveryOutcome::RolledBack);
            outcome
        }
    };
    pair.retire(outcome).unwrap();
    pair.release().unwrap();
    drop(pair);
    assert!(!active.exists());
    RetiredSource {
        root,
        target,
        retired,
        committed,
    }
}

#[test]
fn qualify_intact_retired_sources_before_new_confirmation() {
    for committed in [false, true] {
        let source = retired_source(committed);
        assert_eq!(
            fs::read(&source.target).unwrap(),
            if committed { b"modified" } else { b"original" }
        );
        assert_eq!(
            fs::read(source.retired.join("intent")).unwrap(),
            b"fixed-source-intent"
        );
        let journal = fs::read(source.retired.join("journal")).unwrap();
        assert_eq!(&journal[..16], b"TCIMG001\x08\0\0\0\0\0\0\0");
        assert_eq!(&journal[48..64], b"originalmodified");
        let mut memory = Memory(Cursor::new(journal));
        let checked = inspect_v1(&mut memory, &mut budget(), &|| false).unwrap();
        assert_eq!(checked.before_bytes(), b"original");
        assert_eq!(checked.after_bytes(), b"modified");
        assert_eq!(
            checked.state(),
            if source.committed {
                V1State::Committed
            } else {
                V1State::Pending
            }
        );
        assert_eq!(fs::read_dir(source.root.path()).unwrap().count(), 2);
    }
}

#[path = "retired_confirmation/api.rs"]
mod api;
