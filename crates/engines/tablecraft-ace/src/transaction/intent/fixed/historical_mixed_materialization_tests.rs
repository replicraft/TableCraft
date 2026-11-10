//! Independent request framing and replay for mixed historical Number growth.
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
    fixtures::access::canonical_created_number::mixed::after_off::populated_mixed::{
        self as raw, Case,
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
    let updates = vec![FixedScalarUpdate::new(0, Value::Null, Value::Int32(37))];
    vec![
        NativeFixedIntentGroup::new_historical_present_int32(
            "Bonus",
            updates,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn literal(before: &[u8]) -> Vec<u8> {
    let mut wire = b"TCNI0013".to_vec();
    wire.extend(121_u32.to_le_bytes());
    wire.extend([13, 0, 0, 0]);
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
fn mixed_presence_selects_only_profile13_and_reproduces_the_complete_raw_oracle() {
    for case in [Case::Text, Case::Binary, Case::LaterAbsentText] {
        let [_, before] = raw::pair(case);
        let expected = raw::after(&before, 0, 37);
        let plan = image(&before, 31)
            .plan_recoverable_historical_fixed_updates("Table1", groups(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16HistoricalMixedNumberMaterialization
        );
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            literal(&before)
        );

        let fresh = plan
            .requalify_with_budgets(image(&before, 32), &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(fresh.after_bytes(), expected);
        let decoded =
            NativeUpdateIntent::decode(&literal(&before), &mut budget(), &|| false).unwrap();
        assert_eq!(
            decoded.encode(&mut budget(), &|| false).unwrap(),
            literal(&before)
        );

        for (magic, profile) in [(b"TCNI0011", 11_u16), (b"TCNI0012", 12)] {
            let mut crossed = literal(&before);
            crossed[..8].copy_from_slice(magic);
            crossed[12..14].copy_from_slice(&profile.to_le_bytes());
            let old = NativeUpdateIntent::decode(&crossed, &mut budget(), &|| false).unwrap();
            assert!(
                crate::transaction::update_rebuild::rebuild(
                    image(&before, 33),
                    &old,
                    &mut budget(),
                    &|| false,
                )
                .is_err(),
                "older historical profiles must not acquire mixed-row growth"
            );
        }
    }
}

#[test]
fn mixed_version_and_profile_must_agree_before_owned_record_allocation() {
    let [_, before] = raw::pair(Case::Text);
    let mut crossed = literal(&before);
    crossed[12..14].copy_from_slice(&12_u16.to_le_bytes());
    assert!(matches!(
        NativeUpdateIntent::decode(&crossed, &mut ResourceBudget::new(0, 100, 100_000), &|| {
            false
        }),
        Err(NativeIntentError::UnsupportedProfile)
    ));
}

#[test]
fn cold_mixed_recovery_preserves_variable_bytes_and_checks_the_full_journal_image() {
    for case in [Case::Text, Case::Binary, Case::LaterAbsentText] {
        let [_, before] = raw::pair(case);
        let after = raw::after(&before, 0, 37);
        for committed in [false, true] {
            // NO_CLONE: Recovery mutates a storage copy while the complete original remains its byte oracle.
            let mut target = MemoryStorage::new(before.clone());
            let mut journal = MemoryStorage::new(Vec::new());
            let plan = image(&before, 34)
                .plan_recoverable_historical_mixed_number_materialization(
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
                target.bytes.get_mut()[8 * 4096..9 * 4096]
                    .copy_from_slice(&after[8 * 4096..9 * 4096]);
            }
            let checked = journal::inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let intent =
                NativeUpdateIntent::decode(&literal(&before), &mut budget(), &|| false).unwrap();
            qualify_native_update_recovery(
                checked,
                intent,
                CatalogScope::new(35),
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
        // NO_CLONE: The refusal must preserve the independent original while journal setup owns mutable storage.
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
                CatalogScope::new(36),
                &mut budget(),
                &|| false
            ),
            Err(NativeRecoveryQualificationError::ImageMismatch)
        ));
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &before);
    }
}
