#![cfg(feature = "journal")]

#[path = "mutation/fixture.rs"]
mod fixture;
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
use tablecraft_testkit::access as support;

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use tablecraft_ace::{Int32Update, NativeImage, recover_native_edit};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::{
    ExclusiveFile, IoError, SharedFile,
    journal::{DurableStorage, JournalError, RecoveryOutcome},
};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 10_000_000)
}

struct Device {
    bytes: Cursor<Vec<u8>>,
    fail_prefix: Option<usize>,
}
impl Device {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Cursor::new(bytes),
            fail_prefix: None,
        }
    }
}
impl Read for Device {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(bytes)
    }
}
impl Seek for Device {
    fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(offset)
    }
}
impl Write for Device {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(prefix) = self.fail_prefix.take() {
            self.bytes.write_all(&bytes[..prefix.min(bytes.len())])?;
            return Err(io::Error::other("injected after-side-effect failure"));
        }
        self.bytes.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl DurableStorage for Device {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.bytes.get_ref().len() as u64)
    }
    fn sync_storage(&mut self) -> Result<(), IoError> {
        Ok(())
    }
}

fn plan(bytes: &[u8]) -> tablecraft_ace::NativeEditPlan {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .plan_int32_updates(
        "Items",
        "Score",
        &[Int32Update::new(0, 10, 0x12345678)],
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn indexed_images_commit_reopen_and_roll_back_partial_field_index_writes_together() {
    let before = index_fixture::node_database();
    let indexed_plan = || {
        NativeImage::capture(
            &mut Cursor::new(&before),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .plan_int32_updates(
            "Items",
            "Score",
            &[Int32Update::new(1, 20, 35)],
            &mut budget(),
            &|| false,
        )
        .unwrap()
    };
    let expected = indexed_plan().after_bytes().to_vec();
    let mut target = Device::new(before.clone());
    let mut journal = Device::new(Vec::new());
    indexed_plan()
        .prepare(&mut target, &mut journal, &mut budget(), &|| false)
        .unwrap()
        .commit(&mut budget(), &|| false)
        .unwrap();
    let (outcome, image) = recover_native_edit(
        &mut target,
        &mut journal,
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(outcome, RecoveryOutcome::Committed);
    assert_eq!(image.as_bytes(), expected);
    assert!(
        tablecraft_ace::check_table(
            &mut image.reader(),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .is_ok()
    );
    // Fail before the root, then after its new separator but before either
    // leaf. Each mixed state must be recoverable as one whole-image operation.
    for prefix in [13 * 4096, 14 * 4096, 16 * 4096] {
        let mut target = Device::new(before.clone());
        let mut journal = Device::new(Vec::new());
        target.fail_prefix = Some(prefix);
        assert!(
            indexed_plan()
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .is_err()
        );
        assert_ne!(target.bytes.get_ref(), &before);
        assert_ne!(target.bytes.get_ref(), &expected);
        assert!(
            tablecraft_ace::check_table(
                &mut Cursor::new(target.bytes.get_ref()),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false
            )
            .is_err()
        );
        let (outcome, image) = recover_native_edit(
            &mut target,
            &mut journal,
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(outcome, RecoveryOutcome::RolledBack);
        assert_eq!(image.as_bytes(), before);
        assert!(
            tablecraft_ace::check_table(
                &mut image.reader(),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false
            )
            .is_ok()
        );
    }
    // No-op plans also retain a complete valid multi-page index through replay.
    let unchanged = index_fixture::node_database();
    let noop = NativeImage::capture(
        &mut Cursor::new(&unchanged),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .plan_int32_updates(
        "Items",
        "Score",
        &[Int32Update::new(1, 20, 20)],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut target = Device::new(unchanged.clone());
    let mut journal = Device::new(Vec::new());
    assert_eq!(
        noop.prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap()
            .commit(&mut budget(), &|| false)
            .unwrap(),
        0
    );
    let (outcome, image) = recover_native_edit(
        &mut target,
        &mut journal,
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(outcome, RecoveryOutcome::Committed);
    assert_eq!(image.as_bytes(), unchanged);
}

#[test]
fn native_commit_failure_recovers_the_entire_before_image_repeatably() {
    let before = fixture::database();
    let initial = plan(&before);
    let after = initial.after_bytes().to_vec();
    let mut target = Device::new(before.clone());
    let mut journal = Device::new(Vec::new());
    let prepared = initial
        .prepare(&mut target, &mut journal, &mut budget(), &|| false)
        .unwrap();
    // End the borrow without cleanup, as an interrupted prepared transaction.
    drop(prepared);
    let (_, image) = recover_native_edit(
        &mut target,
        &mut journal,
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(image.as_bytes(), before);
    // A new transaction needs a fresh journal, never implicit reuse.
    journal = Device::new(Vec::new());
    target.fail_prefix = Some(12 * 4096 + 4096 - 11 + 6 + 1);
    assert!(
        plan(&before)
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap()
            .commit(&mut budget(), &|| false)
            .is_err()
    );
    assert_ne!(target.bytes.get_ref(), &before);
    assert_ne!(target.bytes.get_ref(), &after);
    for _ in 0..2 {
        let (outcome, image) = recover_native_edit(
            &mut target,
            &mut journal,
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(outcome, RecoveryOutcome::RolledBack);
        assert_eq!(image.as_bytes(), before);
    }
}

#[test]
fn stale_targets_and_reused_journals_fail_before_native_target_changes() {
    let before = fixture::database();
    let mut stale = before.clone();
    stale[13 * 4096] ^= 1;
    let mut target = Device::new(stale.clone());
    let mut journal = Device::new(Vec::new());
    assert_eq!(
        plan(&before)
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .err(),
        Some(JournalError::TargetMismatch)
    );
    assert_eq!(target.bytes.get_ref(), &stale);
    assert!(journal.bytes.get_ref().is_empty());
    target = Device::new(before.clone());
    assert_eq!(
        plan(&before)
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap()
            .commit(&mut budget(), &|| false)
            .unwrap(),
        1
    );
    let committed = target.bytes.get_ref().clone();
    let (outcome, image) = recover_native_edit(
        &mut target,
        &mut journal,
        CatalogScope::new(2),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(outcome, RecoveryOutcome::Committed);
    assert_eq!(image.as_bytes(), committed);
    assert_eq!(
        plan(&before)
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .err(),
        Some(JournalError::JournalNotEmpty)
    );
    assert_eq!(target.bytes.get_ref(), &committed);
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Files(PathBuf);
impl Drop for Files {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn locked_native_files_commit_close_reopen_and_recover_with_retained_journal() {
    let files = Files(std::env::temp_dir().join(format!(
        "tablecraft-native-write-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir(&files.0).unwrap();
    let target_path = files.0.join("target.mdb");
    let journal_path = files.0.join("journal");
    let before = fixture::with_rule("ValidationRule", ">=-100 And <=100");
    for (path, bytes) in [(&target_path, before.as_slice()), (&journal_path, &[])] {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
    }
    #[cfg(unix)]
    File::open(&files.0).unwrap().sync_all().unwrap();
    // Windows CI exercises reopen/locks, not durable namespace qualification.
    #[cfg(not(unix))]
    let _ = File::open(&target_path).unwrap();
    let mut target = ExclusiveFile::open(&target_path).unwrap();
    let mut journal = ExclusiveFile::open(&journal_path).unwrap();
    assert!(matches!(
        SharedFile::open(&target_path),
        Err(IoError::LockBusy)
    ));
    let image =
        NativeImage::capture(&mut target, CatalogScope::new(3), &mut budget(), &|| false).unwrap();
    let plan = image
        .plan_int32_updates(
            "Items",
            "Score",
            &[Int32Update::new(2, 30, -8)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let expected = plan.after_bytes().to_vec();
    plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
        .unwrap()
        .commit(&mut budget(), &|| false)
        .unwrap();
    target.close().unwrap();
    journal.close().unwrap();
    let mut target = ExclusiveFile::open(&target_path).unwrap();
    let mut journal = ExclusiveFile::open(&journal_path).unwrap();
    let (outcome, image) = recover_native_edit(
        &mut target,
        &mut journal,
        CatalogScope::new(4),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(outcome, RecoveryOutcome::Committed);
    assert_eq!(image.as_bytes(), expected);
    assert!(journal.length().unwrap() > before.len() as u64);
    target.close().unwrap();
    journal.close().unwrap();
    assert_eq!(fs::read(&target_path).unwrap(), expected);
}
