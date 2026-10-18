use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use tablecraft_ace::{InspectError, OfflineReadTransaction, check_table};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::{ExclusiveFile, IoError};
use tablecraft_model::CatalogScope;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Copy(PathBuf);
impl Copy {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-read-transaction-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../docs/examples/Access_Example_VBA.accdb"),
            &path,
        )
        .unwrap();
        Self(path)
    }
}
impl Drop for Copy {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}

#[test]
fn offline_transactions_supply_repeatable_native_reads_and_hold_cooperating_locks() {
    let copy = Copy::new();
    let original = fs::read(&copy.0).unwrap();
    let mut tx = OfflineReadTransaction::open(&copy.0, &mut budget(), &|| false).unwrap();
    assert_eq!(tx.header().page_count(), 827);
    let report = check_table(
        &mut tx,
        CatalogScope::new(1),
        "ROULETTE_TB",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.rows(), 38);
    assert!(matches!(
        ExclusiveFile::open(&copy.0),
        Err(IoError::LockBusy)
    ));
    let mut second = OfflineReadTransaction::open(&copy.0, &mut budget(), &|| false).unwrap();
    let mut bytes = Vec::new();
    second.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, original);
    tx.seek(SeekFrom::Start(0)).unwrap();
    bytes.clear();
    tx.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, original);
    drop(tx);
    drop(second);
    assert!(ExclusiveFile::open(&copy.0).is_ok());
    assert_eq!(fs::read(&copy.0).unwrap(), original);
}

#[test]
fn cancellation_limits_and_busy_open_release_every_acquired_lock() {
    let copy = Copy::new();
    assert_eq!(
        OfflineReadTransaction::open(&copy.0, &mut budget(), &|| true).err(),
        Some(InspectError::Cancelled)
    );
    assert!(matches!(
        OfflineReadTransaction::open(&copy.0, &mut ResourceBudget::new(100, 1, 100), &|| false),
        Err(InspectError::Limit(_))
    ));
    let lock = ExclusiveFile::open(&copy.0).unwrap();
    assert_eq!(
        OfflineReadTransaction::open(&copy.0, &mut budget(), &|| false).err(),
        Some(InspectError::LockBusy)
    );
    drop(lock);
    assert!(OfflineReadTransaction::open(&copy.0, &mut budget(), &|| false).is_ok());
}
