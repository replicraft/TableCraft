//! Independent request bytes and cold recovery for unchanged historical framing.
use super::NativeFixedIntentGroup;
#[cfg(feature = "ace16-create")]
use crate::qualify_native_update_recovery;
use crate::{
    FixedScalarUpdate, NativeImage, NativeIntentError, NativeUpdateIntent, NativeUpdateProfile,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
#[cfg(feature = "ace16-create")]
use tablecraft_io::journal;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated;
#[cfg(feature = "ace16-create")]
use tablecraft_testkit::support::io::journal::MemoryStorage;

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

#[cfg(feature = "ace16-create")]
fn requests() -> Vec<NativeFixedIntentGroup> {
    let updates = vec![
        FixedScalarUpdate::new(0, Value::Int32(42), Value::Int32(8)),
        FixedScalarUpdate::new(1, Value::Null, Value::Int32(12)),
        FixedScalarUpdate::new(2, Value::Int32(-7), Value::Null),
    ];
    vec![
        NativeFixedIntentGroup::new_historical_present_int32(
            "Score",
            updates,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn source() -> Vec<u8> {
    let [_, before] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    before
}

fn literal(before: &[u8]) -> Vec<u8> {
    let mut wire = b"TCNI0011".to_vec();
    wire.extend(173_u32.to_le_bytes());
    wire.extend([11, 0, 0, 0]);
    wire.extend((before.len() as u64).to_le_bytes());
    let mut binding = Sha256::new();
    binding.update(b"TCNI0001:BEFORE\0");
    binding.update((before.len() as u64).to_le_bytes());
    binding.update(before);
    wire.extend(binding.finalize());
    wire.extend(6_u32.to_le_bytes());
    wire.extend(1_u32.to_le_bytes());
    wire.extend(3_u32.to_le_bytes());
    wire.extend([0; 4]);
    wire.extend(b"Table1");
    wire.extend(5_u32.to_le_bytes());
    wire.extend(3_u32.to_le_bytes());
    wire.extend([0; 4]);
    wire.extend(b"Score");
    for (row, before, after) in [
        (0_u64, Some(42_i32), Some(8_i32)),
        (1, None, Some(12)),
        (2, Some(-7), None),
    ] {
        wire.extend(row.to_le_bytes());
        for value in [before, after] {
            wire.push(if value.is_some() { 2 } else { 0 });
            wire.extend(u64::from(value.unwrap_or(0) as u32).to_le_bytes());
        }
    }
    assert_eq!(wire.len(), 173);
    wire
}

#[cfg(feature = "ace16-create")]
#[test]
fn closed_wire_rebuilds_the_independent_whole_image_under_fresh_scope() {
    let before = source();
    let expected = populated::update_scores(&before, &[(0, Some(8)), (1, Some(12)), (2, None)]);
    let plan = image(&before, 12)
        .plan_recoverable_historical_present_fixed_updates(
            "Table1",
            requests(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.after_bytes(), expected);
    assert_eq!(plan.changed_rows(), 3);
    let encoded = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert_eq!(encoded, literal(&before));
    let decoded = NativeUpdateIntent::decode(&encoded, &mut budget(), &|| false).unwrap();
    assert_eq!(
        decoded.profile(),
        NativeUpdateProfile::Ace16HistoricalPresentFixed
    );
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), encoded);

    let fresh = plan
        .requalify_with_budgets(image(&before, 13), &mut budget(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(fresh.before_bytes(), before);
    assert_eq!(fresh.after_bytes(), expected);
}

#[test]
fn wire_rejects_crossed_profiles_and_non_int32_cells_before_owned_allocation() {
    let before = source();
    let wire = literal(&before);
    for offset in [8, 14, 68] {
        // NO_CLONE: Each malformed header must start from the untouched independent wire.
        let mut invalid = wire.clone();
        invalid[offset] ^= 1;
        assert!(matches!(
            NativeUpdateIntent::decode(
                &invalid,
                &mut ResourceBudget::new(0, 100, 100_000),
                &|| false
            ),
            Err(NativeIntentError::InvalidRecord)
        ));
    }
    // NO_CLONE: Cross the profile while preserving the exact original record version.
    let mut crossed = wire.clone();
    crossed[12..14].copy_from_slice(&10_u16.to_le_bytes());
    assert!(matches!(
        NativeUpdateIntent::decode(&crossed, &mut ResourceBudget::new(0, 100, 100_000), &|| {
            false
        }),
        Err(NativeIntentError::UnsupportedProfile)
    ));
    // NO_CLONE: Forge a canonical Int64 cell inside otherwise unchanged original request bytes.
    let mut wrong_carrier = wire.clone();
    wrong_carrier[103] = 3;
    assert!(matches!(
        NativeUpdateIntent::decode(
            &wrong_carrier,
            &mut ResourceBudget::new(0, 100, 100_000),
            &|| false,
        ),
        Err(NativeIntentError::InvalidRecord)
    ));

    for value in [Value::Boolean(true), Value::Int64(42), Value::Byte(42)] {
        assert!(matches!(
            NativeFixedIntentGroup::new_historical_present_int32(
                "Score",
                vec![FixedScalarUpdate::new(0, Value::Int32(42), value)],
                &mut budget(),
                &|| false,
            ),
            Err(NativeIntentError::InvalidRecord)
        ));
    }

    let mut old = wire;
    old[..8].copy_from_slice(b"TCNI0004");
    old[12..14].copy_from_slice(&4_u16.to_le_bytes());
    let decoded = NativeUpdateIntent::decode(&old, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.profile(), NativeUpdateProfile::Ace16OrdinaryFixed);
    assert!(
        crate::transaction::update_rebuild::rebuild(
            image(&before, 14),
            &decoded,
            &mut budget(),
            &|| false,
        )
        .is_err(),
        "an older request language must not inherit historical-record admission"
    );
}

#[cfg(feature = "ace16-create")]
#[test]
fn pending_and_committed_cold_recovery_reconstruct_every_original_byte() {
    let before = source();
    let after = populated::update_scores(&before, &[(0, Some(8)), (1, Some(12)), (2, None)]);
    for committed in [false, true] {
        // NO_CLONE: Recovery owns mutable storage while the independent BEFORE remains the oracle.
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let plan = image(&before, 15)
            .plan_recoverable_historical_present_fixed_updates(
                "Table1",
                requests(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
            target.bytes.get_mut()[8 * 4096..9 * 4096].copy_from_slice(&after[8 * 4096..9 * 4096]);
        }
        let checked = journal::inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let intent =
            NativeUpdateIntent::decode(&literal(&before), &mut budget(), &|| false).unwrap();
        qualify_native_update_recovery(
            checked,
            intent,
            CatalogScope::new(16),
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
}

#[cfg(feature = "ace16-create")]
#[test]
fn forged_journal_complements_never_gain_historical_update_authority() {
    let before = source();
    let after = populated::update_scores(&before, &[(0, Some(8)), (1, Some(12)), (2, None)]);
    for offset in [7 * 4096 + 16, 8 * 4096 + 4095, 9 * 4096 + 481] {
        // NO_CLONE: Every forged image changes one independent complement byte only.
        let mut forged = after.clone();
        forged[offset] ^= 4;
        // NO_CLONE: Journal preparation owns mutable target storage while the original remains the oracle.
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_expected(
                &mut target,
                &mut journal,
                &before,
                &forged,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let intent =
            NativeUpdateIntent::decode(&literal(&before), &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_update_recovery(
                checked,
                intent,
                CatalogScope::new(17),
                &mut budget(),
                &|| false,
            ),
            Err(crate::NativeRecoveryQualificationError::ImageMismatch)
        ));
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &before);
    }
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn data_only_historical_wire_cannot_reconstruct_without_the_native_writer_capability() {
    let before = source();
    let wire = literal(&before);
    let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
    assert_eq!(
        decoded.profile(),
        NativeUpdateProfile::Ace16HistoricalPresentFixed
    );
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
    assert!(matches!(
        crate::transaction::update_rebuild::rebuild(
            image(&before, 18),
            &decoded,
            &mut budget(),
            &|| false
        ),
        Err(crate::NativeRecoveryQualificationError::Intent(
            NativeIntentError::UnsupportedProfile
        ))
    ));
}
