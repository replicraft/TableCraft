//! Sealed DELETE proof retains full-image and original-request identity.
use super::*;
use crate::{EditError, InspectError, NativeIntentError, RowDelete};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::Resource;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{canonical_creation, canonical_native17_creation};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(257),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn plan(before: &[u8], ordinals: &[usize]) -> RecoverableNativeDeletePlan {
    let requests = ordinals
        .iter()
        .map(|ordinal| {
            let slot = 8 * 4096 + 14 + 2 * *ordinal;
            let start = usize::from(u16::from_le_bytes(
                before[slot..slot + 2].try_into().unwrap(),
            ));
            RowDelete::new(
                *ordinal,
                before[8 * 4096 + start..8 * 4096 + start + 11].to_vec(),
            )
        })
        .collect();
    image(before)
        .plan_recoverable_ordinary_row_deletes("Items", requests, &mut budget(), &|| false)
        .unwrap()
}

#[test]
fn valid_and_empty_rebuilds_preserve_independent_complete_images_and_intent() {
    let before = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Boundary).bytes,
    );
    for ordinals in [vec![62_usize, 63], vec![]] {
        let mut expected = before.clone();
        for ordinal in &ordinals {
            let slot = 8 * 4096 + 14 + 2 * *ordinal;
            let original = u16::from_le_bytes(before[slot..slot + 2].try_into().unwrap());
            expected[slot..slot + 2].copy_from_slice(&(original | 0xc000).to_le_bytes());
        }
        expected[7 * 4096 + 16..7 * 4096 + 20]
            .copy_from_slice(&(64_u32 - ordinals.len() as u32).to_le_bytes());
        let reviewed = plan(&before, &ordinals);
        let intent = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
        let fresh = reviewed
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(fresh.before_bytes(), before);
        assert_eq!(fresh.after_bytes(), expected);
        assert_eq!(fresh.deleted_rows(), ordinals.len());
        assert_eq!(
            fresh.intent().encode(&mut budget(), &|| false).unwrap(),
            intent
        );
    }
}

#[test]
fn different_complete_source_and_different_original_intent_refuse() {
    let before = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Boundary).bytes,
    );
    let mut stale = before.clone();
    let tail = stale.len() - 1;
    stale[tail] ^= 1;
    assert!(matches!(
        plan(&before, &[1]).requalify_with_budgets(
            image(&stale),
            &mut budget(),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));

    let mut reviewed = plan(&before, &[1]);
    reviewed.intent = plan(&before, &[0]).intent;
    assert!(matches!(
        reviewed.requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
}

#[test]
fn independent_native_and_proof_limits_and_terminal_cancel_publish_no_candidate() {
    let before = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Boundary).bytes,
    );
    for native_empty in [false, true] {
        let mut native = if native_empty {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let mut proof = if native_empty {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        assert!(
            plan(&before, &[1])
                .requalify_with_budgets(image(&before), &mut native, &mut proof, &|| false)
                .is_err()
        );
    }
    let polls = Cell::new(0);
    let mut native = budget();
    let mut proof = budget();
    plan(&before, &[1])
        .requalify_with_budgets(image(&before), &mut native, &mut proof, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    let total = polls.get();
    assert!(proof.remaining(Resource::Steps) < budget().remaining(Resource::Steps));
    assert!(native.remaining(Resource::Bytes) < budget().remaining(Resource::Bytes));
    for stop in [1, total / 2, total] {
        let observed = Cell::new(0);
        assert!(matches!(
            plan(&before, &[1]).requalify_with_budgets(
                image(&before),
                &mut budget(),
                &mut budget(),
                &|| {
                    observed.set(observed.get() + 1);
                    observed.get() >= stop
                }
            ),
            Err(
                NativeRecoveryQualificationError::Inspect(InspectError::Cancelled)
                    | NativeRecoveryQualificationError::Edit(EditError::Inspect(
                        InspectError::Cancelled
                    ))
                    | NativeRecoveryQualificationError::Intent(NativeIntentError::Inspect(
                        InspectError::Cancelled
                    ))
            )
        ));
    }
}
