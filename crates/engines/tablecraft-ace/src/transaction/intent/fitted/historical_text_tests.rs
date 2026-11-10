//! Independent TCNI0015 bytes and cold reconstruction of historical Text records.
use crate::{
    NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage,
    NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile, SlotScalarUpdate,
    qualify_native_update_recovery,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::journal;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::mixed::after_off::populated_text as raw,
    support::io::journal::MemoryStorage,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn groups(value: &str) -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new(
            "NewLabel",
            NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
                0,
                Value::Null,
                Value::Text(value.into()),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn sources() -> Vec<Vec<u8>> {
    vec![
        raw::pair("NewLabel", &[Some(42), None, Some(-7)])[1].to_vec(),
        raw::mixed_pair()[1].to_vec(),
        raw::forwarded_pair()[1].to_vec(),
    ]
}

fn literal(before: &[u8], value: &str) -> Vec<u8> {
    let mut wire = b"TCNI0015".to_vec();
    wire.extend((134_u32 + value.len() as u32).to_le_bytes());
    wire.extend([15, 0, 0, 0]);
    wire.extend((before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    wire.extend(hash.finalize());
    for number in [6_u32, 1, 1, 0] {
        wire.extend(number.to_le_bytes());
    }
    wire.extend(b"Table1");
    wire.extend((56_u32 + value.len() as u32).to_le_bytes());
    wire.extend(8_u32.to_le_bytes());
    wire.extend(1_u32.to_le_bytes());
    wire.extend([1, 0, 0, 0]);
    wire.extend([0; 4]);
    wire.extend(b"NewLabel");
    wire.extend((28_u32 + value.len() as u32).to_le_bytes());
    wire.extend([0; 8]);
    wire.extend([0; 8]);
    wire.extend([6, 0, 0, 0]);
    wire.extend((value.len() as u32).to_le_bytes());
    wire.extend(value.as_bytes());
    assert_eq!(wire.len(), 134 + value.len());
    wire
}

#[test]
fn original_text_request_has_distinct_wire_and_fresh_scoped_reconstruction() {
    for before in sources() {
        for value in ["", "日本語 🦀", "compressed label"] {
            let expected = raw::after(&before, 0, value);
            let reviewed = image(&before, 71)
                .plan_recoverable_historical_short_text_materialization(
                    "Table1",
                    groups(value),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                reviewed.intent().profile(),
                NativeUpdateProfile::Ace16HistoricalShortTextMaterialization
            );
            assert_eq!(reviewed.changed_rows(), 1);
            assert_eq!(reviewed.after_bytes(), expected);
            let wire = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(wire, literal(&before, value));
            assert!(NativeFittedEditIntent::decode(&wire, &mut budget(), &|| false).is_err());
            let fresh = reviewed
                .requalify_with_budgets(image(&before, 72), &mut budget(), &mut budget(), &|| false)
                .unwrap();
            assert_eq!(fresh.after_bytes(), expected);
            let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);

            for (magic, profile) in [(b"TCNI0005", 5_u16), (b"TCNI0006", 6), (b"TCNI0007", 7)] {
                let mut crossed = literal(&before, value);
                crossed[..8].copy_from_slice(magic);
                crossed[12..14].copy_from_slice(&profile.to_le_bytes());
                let old = NativeUpdateIntent::decode(&crossed, &mut budget(), &|| false).unwrap();
                assert!(
                    crate::transaction::update_rebuild::rebuild(
                        image(&before, 73),
                        &old,
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn cold_text_materialization_recovery_requires_complete_rebuilt_images() {
    for before in sources() {
        let after = raw::after(&before, 0, "saved text");
        for committed in [false, true] {
            // NO_CLONE: Recovery mutates independent storage while BEFORE remains the whole-image oracle.
            let mut target = MemoryStorage::new(before.clone());
            let mut log = MemoryStorage::new(Vec::new());
            let plan = image(&before, 74)
                .plan_recoverable_historical_short_text_materialization(
                    "Table1",
                    groups("saved text"),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let prepared = plan
                .prepare(&mut target, &mut log, &mut budget(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                target.bytes.get_mut()[8 * 4096..9 * 4096]
                    .copy_from_slice(&after[8 * 4096..9 * 4096]);
            }
            let checked = journal::inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
            let intent =
                NativeUpdateIntent::decode(&literal(&before, "saved text"), &mut budget(), &|| {
                    false
                })
                .unwrap();
            qualify_native_update_recovery(
                checked,
                intent,
                CatalogScope::new(75),
                &mut budget(),
                &|| false,
            )
            .unwrap()
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap();
            assert_eq!(
                target.bytes.get_ref(),
                if committed { &after } else { &before }
            );
        }
        // NO_CLONE: Corrupt one candidate byte without altering the independent AFTER oracle.
        let mut forged = after.clone();
        *forged.last_mut().unwrap() ^= 1;
        // NO_CLONE: Generic journal preparation owns mutable storage; original bytes remain the refusal oracle.
        let mut target = MemoryStorage::new(before.clone());
        let mut log = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_expected(
                &mut target,
                &mut log,
                &before,
                &forged,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
        let intent =
            NativeUpdateIntent::decode(&literal(&before, "saved text"), &mut budget(), &|| false)
                .unwrap();
        assert!(matches!(
            qualify_native_update_recovery(
                checked,
                intent,
                CatalogScope::new(76),
                &mut budget(),
                &|| false
            ),
            Err(NativeRecoveryQualificationError::ImageMismatch)
        ));
        assert_eq!(target.writes, 0);
    }
}
