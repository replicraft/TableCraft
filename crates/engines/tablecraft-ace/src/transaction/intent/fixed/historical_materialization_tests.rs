//! Original request and complete recovery proof for absent-field materialization.
use super::NativeFixedIntentGroup;
use crate::{
    FixedScalarUpdate, NativeImage, NativeIntentError, NativeRecoveryQualificationError,
    NativeUpdateIntent, NativeUpdateProfile, qualify_native_update_recovery,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::journal;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::mixed::after_off::populated::{
        self, materialization,
    },
    support::io::journal::MemoryStorage,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(23),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn source() -> Vec<u8> {
    let [_, before] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    before
}

fn groups() -> Vec<NativeFixedIntentGroup> {
    vec![
        NativeFixedIntentGroup::new_historical_present_int32(
            "Bonus",
            vec![FixedScalarUpdate::new(0, Value::Null, Value::Int32(37))],
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn literal(before: &[u8]) -> Vec<u8> {
    let mut wire = b"TCNI0012".to_vec();
    wire.extend(121_u32.to_le_bytes());
    wire.extend([12, 0, 0, 0]);
    wire.extend((before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    wire.extend(hash.finalize());
    wire.extend(6_u32.to_le_bytes());
    wire.extend(1_u32.to_le_bytes());
    wire.extend(1_u32.to_le_bytes());
    wire.extend([0; 4]);
    wire.extend(b"Table1");
    wire.extend(5_u32.to_le_bytes());
    wire.extend(1_u32.to_le_bytes());
    wire.extend([0; 4]);
    wire.extend(b"Bonus");
    wire.extend([0; 8]);
    wire.extend([0; 9]);
    wire.push(2);
    wire.extend(37_u64.to_le_bytes());
    assert_eq!(wire.len(), 121);
    wire
}

#[test]
fn actual_presence_selects_profile12_and_requalifies_the_independent_placement_image() {
    let before = source();
    let expected = materialization::after(&before, 0, 2, 37);
    let plan = image(&before)
        .plan_recoverable_historical_fixed_updates("Table1", groups(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        plan.intent().profile(),
        NativeUpdateProfile::Ace16HistoricalNumberMaterialization
    );
    assert_eq!(plan.changed_rows(), 1);
    assert_eq!(plan.after_bytes(), expected);
    assert_eq!(
        plan.intent().encode(&mut budget(), &|| false).unwrap(),
        literal(&before)
    );

    let plan = plan
        .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), expected);
    let decoded = NativeUpdateIntent::decode(&literal(&before), &mut budget(), &|| false).unwrap();
    assert_eq!(
        decoded.encode(&mut budget(), &|| false).unwrap(),
        literal(&before)
    );

    let mut previous = literal(&before);
    previous[..8].copy_from_slice(b"TCNI0011");
    previous[12..14].copy_from_slice(&11_u16.to_le_bytes());
    let old = NativeUpdateIntent::decode(&previous, &mut budget(), &|| false).unwrap();
    assert_eq!(
        old.profile(),
        NativeUpdateProfile::Ace16HistoricalPresentFixed
    );
    assert!(crate::transaction::update_rebuild::rebuild(
        image(&before), &old, &mut budget(), &|| false,
    ).is_err(), "profile11 must not acquire field materialization");
}

#[test]
fn crossed_materialization_record_profile_refuses_before_owned_buffers() {
    let before = source();
    let mut wire = literal(&before);
    wire[12..14].copy_from_slice(&11_u16.to_le_bytes());
    assert!(matches!(
        NativeUpdateIntent::decode(&wire, &mut ResourceBudget::new(0, 100, 100_000), &|| false),
        Err(NativeIntentError::UnsupportedProfile)
    ));
}

#[test]
fn cold_materialization_recovery_restores_or_commits_exact_forwarded_roots() {
    let before = source();
    let after = materialization::after(&before, 0, 2, 37);
    for committed in [false, true] {
        // NO_CLONE: Recovery mutates the storage while the complete independent original remains the oracle.
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let plan = image(&before)
            .plan_recoverable_historical_number_materialization(
                "Table1",
                groups(),
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
            CatalogScope::new(24),
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

#[test]
fn forged_materialization_metadata_never_authorizes_recovery_writes() {
    let before = source();
    let mut forged = materialization::after(&before, 0, 2, 37);
    forged[7 * 4096 + 16] ^= 1;
    // NO_CLONE: Keep the independently authored BEFORE while the journal owns its mutable target copy.
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
    let intent = NativeUpdateIntent::decode(&literal(&before), &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_update_recovery(
            checked,
            intent,
            CatalogScope::new(25),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
