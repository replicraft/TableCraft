//! Independent request and cold-recovery checks for repeated historical growth.
use super::NativeFixedIntentGroup;
use crate::{
    FixedScalarUpdate, NativeImage, NativeRecoveryQualificationError, NativeUpdateIntent,
    NativeUpdateProfile, qualify_native_update_recovery,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::journal;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::mixed::after_off::{
        populated::materialization::forwarded,
        populated_mixed::{self, Case},
    },
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

fn groups() -> Vec<NativeFixedIntentGroup> {
    vec![
        NativeFixedIntentGroup::new_historical_present_int32(
            "SecondBonus",
            vec![FixedScalarUpdate::new(0, Value::Null, Value::Int32(59))],
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn fixtures() -> Vec<(Vec<u8>, Vec<u8>)> {
    let [_, before] = forwarded::pair();
    let after = forwarded::after_second(&before, 59);
    let mut fixtures = vec![(before, after)];
    for case in [Case::Text, Case::Binary, Case::LaterAbsentText] {
        let [_, before] = populated_mixed::forwarded_pair(case);
        let after = populated_mixed::forwarded_after(&before, 59);
        fixtures.push((before, after));
    }
    fixtures
}

fn literal(before: &[u8]) -> Vec<u8> {
    let mut wire = b"TCNI0014".to_vec();
    wire.extend(127_u32.to_le_bytes());
    wire.extend([14, 0, 0, 0]);
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
    wire.extend(11_u32.to_le_bytes());
    wire.extend(1_u32.to_le_bytes());
    wire.extend([0; 4]);
    wire.extend(b"SecondBonus");
    wire.extend([0; 8]);
    wire.extend([0; 9]);
    wire.push(2);
    wire.extend(59_u64.to_le_bytes());
    assert_eq!(wire.len(), 127);
    wire
}

#[test]
fn forwarded_growth_selects_profile14_without_expanding_earlier_languages() {
    for (before, after) in fixtures() {
        let plan = image(&before, 41)
            .plan_recoverable_historical_fixed_updates("Table1", groups(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16HistoricalForwardedNumberMaterialization
        );
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(plan.after_bytes(), after);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            literal(&before)
        );
        let fresh = plan
            .requalify_with_budgets(image(&before, 42), &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(fresh.after_bytes(), after);

        for (magic, profile) in [(b"TCNI0011", 11_u16), (b"TCNI0012", 12), (b"TCNI0013", 13)] {
            let mut crossed = literal(&before);
            crossed[..8].copy_from_slice(magic);
            crossed[12..14].copy_from_slice(&profile.to_le_bytes());
            let old = NativeUpdateIntent::decode(&crossed, &mut budget(), &|| false).unwrap();
            assert!(
                crate::transaction::update_rebuild::rebuild(
                    image(&before, 43),
                    &old,
                    &mut budget(),
                    &|| false,
                )
                .is_err()
            );
        }
    }
}

#[test]
fn forwarded_growth_cold_recovery_rebuilds_both_complete_images() {
    for (before, after) in fixtures() {
        for committed in [false, true] {
            // NO_CLONE: Recovery owns mutable storage; the original remains an independent whole-image oracle.
            let mut target = MemoryStorage::new(before.clone());
            let mut log = MemoryStorage::new(Vec::new());
            let plan = image(&before, 44)
                .plan_recoverable_historical_forwarded_number_materialization(
                    "Table1",
                    groups(),
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
                NativeUpdateIntent::decode(&literal(&before), &mut budget(), &|| false).unwrap();
            qualify_native_update_recovery(
                checked,
                intent,
                CatalogScope::new(45),
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

        let mut forged = after;
        forged[8 * 4096 + 20] ^= 1;
        // NO_CLONE: Journal setup owns storage while rejection is checked against the unchanged original.
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
            NativeUpdateIntent::decode(&literal(&before), &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_update_recovery(
                checked,
                intent,
                CatalogScope::new(46),
                &mut budget(),
                &|| false
            ),
            Err(NativeRecoveryQualificationError::ImageMismatch)
        ));
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &before);
    }
}
