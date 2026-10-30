use super::Unix;
use crate::IoError;
use crate::publication::protocol;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use tablecraft_core::limits::ResourceBudget;

static ENTROPY_CALLS: AtomicUsize = AtomicUsize::new(0);

fn controlled_entropy(bytes: &mut [u8; 16]) -> Result<(), IoError> {
    let call = ENTROPY_CALLS.fetch_add(1, Ordering::SeqCst);
    assert!(call < 2, "unexpected entropy retry");
    bytes.fill(if call == 0 { 0 } else { 1 });
    Ok(())
}

#[test]
fn required_output_reserved_final_nonce_is_skipped_before_stage_creation() {
    let root = tempfile::tempdir().unwrap();
    // Independent literal: the first injected nonce would occupy this final
    // name unless the actual create loop rejects it before mkdirat.
    let output = root
        .path()
        .join(".tablecraft-stage-00000000000000000000000000000000");
    let mut budget = ResourceBudget::new(1_000_000, 0, 1_000_000);
    let mut backend = Unix::capture(&output, &mut budget, &|| false).unwrap();
    backend.entropy_override = Some(controlled_entropy);

    let receipt = protocol::publish(backend, b"COPY", &mut budget, &|| false).unwrap();
    assert!(receipt.namespace_synced());
    assert_eq!(ENTROPY_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(fs::read(&output).unwrap(), b"COPY");
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
