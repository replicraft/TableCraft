#![cfg(all(
    feature = "filesystem-journal",
    any(target_os = "macos", target_os = "linux", windows)
))]

use std::fs;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::filesystem::{
    OfflineJournalPair, OfflineTarget, PairError, TargetLockState,
};

const ORIGINAL: &[u8] = b"original target";
const INTENT: &[u8] = b"opaque host action";

fn budget() -> ResourceBudget {
    ResourceBudget::new(10_000_000, 0, 10_000_000)
}

#[test]
fn explicit_intent_allowance_refuses_before_namespace_and_retains_target() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.accdb");
    let journal = root.path().join("journal");
    fs::write(&path, ORIGINAL).unwrap();
    let target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();

    let failure = OfflineJournalPair::create_with_intent_limit(
        target,
        &journal,
        INTENT.to_vec(),
        INTENT.len() as u64 - 1,
        &mut budget(),
        &|| false,
    )
    .err()
    .expect("the exact intent exceeds the selected allowance");
    assert!(matches!(failure.cause(), PairError::IntentLimit));
    assert!(!failure.incomplete_setup());
    assert!(!journal.exists());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);

    let mut target = failure.into_target();
    assert_eq!(target.lock_state(), TargetLockState::Held);
    target.release().unwrap();
    assert_eq!(fs::read(path).unwrap(), ORIGINAL);
}

#[test]
fn exact_intent_allowance_publishes_the_same_opaque_payload() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.accdb");
    let journal = root.path().join("journal");
    fs::write(&path, ORIGINAL).unwrap();
    let target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();

    let mut pair = OfflineJournalPair::create_with_intent_limit(
        target,
        &journal,
        INTENT.to_vec(),
        INTENT.len() as u64,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(pair.intent_bytes(), INTENT);
    assert!(journal.exists());
    pair.release().unwrap();
    assert_eq!(fs::read(path).unwrap(), ORIGINAL);
}
