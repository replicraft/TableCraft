//! Frozen V1 and explicit V2 source purposes share framing, never admission retries.
use super::{budget, decode, image, literal};
use crate::{AddColumnIntentError, NativeAddColumnIntent};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_testkit::{
    fixtures::access::{canonical_add_column, canonical_created_add_column as raw},
    support::io::journal::MemoryStorage,
};

fn wire(before: &[u8], column: &str) -> Vec<u8> {
    let mut bytes = literal(before, column);
    // Independent V2 version and profile; the original 60-byte body is unchanged.
    bytes[..8].copy_from_slice(b"TCAC0002");
    bytes[12..14].copy_from_slice(&2_u16.to_le_bytes());
    bytes
}

#[test]
fn v2_wire_and_whole_images_preserve_generated_keys_and_original_defaults() {
    let before = raw::default_zero_source();
    for name in ["Extra", "Added 金額"] {
        let after = raw::append(&before, name);
        let plan = image(&before)
            .plan_recoverable_created_add_int32_column("Items", name, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&before, name)
        );
        let fresh = plan
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(fresh.before_bytes(), before);
        assert_eq!(fresh.after_bytes(), after);
        assert!(
            image(&before)
                .plan_recoverable_add_int32_column("Items", name, &mut budget(), &|| false,)
                .is_err()
        );
        let selected = image(&before)
            .plan_recoverable_add_int32_column_for_source("Items", name, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            selected.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&before, name)
        );
    }
    let old = canonical_add_column::source("Extra");
    let selected = image(&old.before)
        .plan_recoverable_add_int32_column_for_source("Items", "Extra", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        selected.intent().encode(&mut budget(), &|| false).unwrap(),
        literal(&old.before, "Extra")
    );
}

#[test]
fn crossed_versions_profiles_and_purpose_substitution_publish_no_authority() {
    let before = raw::default_zero_source();
    let raw = wire(&before, "Extra");
    for (version, profile) in [(b'1', 2_u16), (b'2', 1), (b'3', 2), (b'2', 3)] {
        let mut crossed = raw.clone();
        crossed[7] = version;
        crossed[12..14].copy_from_slice(&profile.to_le_bytes());
        let mut work = ResourceBudget::new(0, 0, 1_000_000);
        assert!(matches!(
            NativeAddColumnIntent::decode(&crossed, &mut work, &|| false),
            Err(AddColumnIntentError::InvalidRecord)
        ));
    }
    let mut plan = image(&before)
        .plan_recoverable_created_add_int32_column("Items", "Extra", &mut budget(), &|| false)
        .unwrap();
    plan.intent = decode(&literal(&before, "Extra"));
    assert!(
        plan.requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false,)
            .is_err()
    );
}

#[test]
fn original_v2_request_controls_pending_committed_and_torn_marker_recovery() {
    let before = raw::default_zero_source();
    let after = raw::append(&before, "Extra");
    for state in 0..4 {
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let plan = image(&before)
            .plan_recoverable_created_add_int32_column("Items", "Extra", &mut budget(), &|| false)
            .unwrap();
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if state == 1 || state == 3 {
            prepared.commit(&mut budget(), &|| false).unwrap();
            if state == 3 {
                let end = journal.bytes.get_ref().len() - 20;
                journal.bytes.get_mut().truncate(end);
            }
        } else {
            drop(prepared);
            if state == 2 {
                target.bytes.get_mut().copy_from_slice(&after);
            }
        }
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = crate::qualify_native_add_column_recovery(
            checked,
            decode(&wire(&before, "Extra")),
            super::SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let outcome = qualified
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            outcome,
            if state == 1 {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            }
        );
        assert_eq!(
            target.bytes.get_ref().as_slice(),
            if state == 1 {
                after.as_slice()
            } else {
                before.as_slice()
            }
        );
    }
}
