//! Disposable real files consume the same public V2 API and fault contract.
use super::{Directory, budget};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use tablecraft_io::{
    ExclusiveFile, IoError,
    journal::{
        DurableStorage, RecoveryOutcome, ResizableDurableStorage, prepare_resizable_expected,
        recover_resizable,
    },
};

fn directory() -> Directory {
    let path = std::env::temp_dir().join(format!(
        "tablecraft-resizable-journal-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    fs::create_dir(&path).unwrap();
    Directory(path)
}

fn create_pair(directory: &Path, before: &[u8]) -> (ExclusiveFile, ExclusiveFile) {
    for (name, bytes) in [("target", before), ("journal", b"".as_slice())] {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(name))
            .unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
    }
    #[cfg(unix)]
    fs::File::open(directory).unwrap().sync_all().unwrap();

    (
        ExclusiveFile::open(&directory.join("target")).unwrap(),
        ExclusiveFile::open(&directory.join("journal")).unwrap(),
    )
}

#[test]
fn real_files_grow_shrink_and_reopen_retained_journals() {
    for (before, after) in [
        (b"old".as_slice(), b"replacement".as_slice()),
        (b"before-tail".as_slice(), b"new".as_slice()),
        (b"".as_slice(), b"created".as_slice()),
        (b"removed".as_slice(), b"".as_slice()),
    ] {
        for committed in [false, true] {
            let directory = directory();
            let (mut target, mut journal) = create_pair(&directory.0, before);
            let prepared = prepare_resizable_expected(
                &mut target,
                &mut journal,
                before,
                after,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
            }
            target.close().unwrap();
            journal.close().unwrap();

            let mut target = ExclusiveFile::open(&directory.0.join("target")).unwrap();
            let mut journal = ExclusiveFile::open(&directory.0.join("journal")).unwrap();
            let expected = if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            };
            for _ in 0..2 {
                assert_eq!(
                    recover_resizable(&mut target, &mut journal, &mut budget(), &|| false).unwrap(),
                    expected
                );
            }
            target.close().unwrap();
            journal.close().unwrap();
            assert_eq!(
                fs::read(directory.0.join("target")).unwrap(),
                if committed { after } else { before }
            );
            assert!(fs::metadata(directory.0.join("journal")).unwrap().len() > 0);
        }
    }
}

#[derive(Clone, Copy)]
enum ResizeFailure {
    Before,
    Partial,
    After,
}

struct FaultFile {
    file: ExclusiveFile,
    failure: ResizeFailure,
}

impl Read for FaultFile {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.file.read(bytes)
    }
}

impl Write for FaultFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("flush is not a durability barrier")
    }
}

impl Seek for FaultFile {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}

impl DurableStorage for FaultFile {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        self.file.storage_len()
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.file.sync_storage()
    }
}

impl ResizableDurableStorage for FaultFile {
    fn resize_storage(&mut self, length: u64) -> Result<(), IoError> {
        match self.failure {
            ResizeFailure::Before => {}
            ResizeFailure::Partial => {
                let current = self.file.storage_len()?;
                assert!(length <= current);
                self.file.resize_storage(length + (current - length) / 2)?;
            }
            ResizeFailure::After => self.file.resize_storage(length)?,
        }
        Err(IoError::Io(io::ErrorKind::Other))
    }
}

#[test]
fn real_resize_failure_before_partial_or_after_effect_requires_recovery() {
    for (before, after) in [
        (b"old".as_slice(), b"replacement".as_slice()),
        (b"before-tail".as_slice(), b"new".as_slice()),
    ] {
        for failure in [
            ResizeFailure::Before,
            ResizeFailure::Partial,
            ResizeFailure::After,
        ] {
            let directory = directory();
            let (file, mut journal) = create_pair(&directory.0, before);
            let mut target = FaultFile { file, failure };
            let prepared = prepare_resizable_expected(
                &mut target,
                &mut journal,
                before,
                after,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(prepared.commit(&mut budget(), &|| false).is_err());
            target.file.close().unwrap();
            journal.close().unwrap();

            let mut target = ExclusiveFile::open(&directory.0.join("target")).unwrap();
            let mut journal = ExclusiveFile::open(&directory.0.join("journal")).unwrap();
            for _ in 0..2 {
                assert_eq!(
                    recover_resizable(&mut target, &mut journal, &mut budget(), &|| false).unwrap(),
                    RecoveryOutcome::RolledBack
                );
            }
            target.close().unwrap();
            journal.close().unwrap();
            assert_eq!(fs::read(directory.0.join("target")).unwrap(), before);
        }
    }
}
