//! The fitted profile replays only its own original request and complete pair.
use super::*;
use tablecraft_testkit::fixtures::access::saved_field_rename::fitting::{self, SourceCase};

fn fitted_plan(before: &[u8], column: &str, replacement: &str) -> RecoverableFieldRenamePlan {
    image(before, SCOPE)
        .plan_recoverable_fitted_field_rename("Table1", column, replacement, &mut budget(), &|| {
            false
        })
        .unwrap()
}

fn fitted_wire(before: &[u8], original: &str, replacement: &str) -> Vec<u8> {
    let mut wire = literal(before, original, replacement);
    wire[12..14].copy_from_slice(&2_u16.to_le_bytes());
    wire
}

#[test]
fn fitted_original_requests_match_independent_raw_pairs_and_never_reinterpret_profile_one() {
    for case in [
        SourceCase::NumberFirst,
        SourceCase::NumberHistory,
        SourceCase::InlineNumberThenText,
        SourceCase::TextNumberText,
    ] {
        for replacement in ["TaxAmount", "X"] {
            let original = fitting::old_name(case);
            let [before, after] = fitting::pair(case, replacement);
            let plan = fitted_plan(&before, original, replacement);
            assert_eq!(plan.before_bytes(), before);
            assert_eq!(plan.after_bytes(), after);
            let encoded = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(encoded, fitted_wire(&before, original, replacement));
            let decoded = decode(&encoded);
            let reconstructed = decoded
                .reconstruct(
                    image(&before, CatalogScope::new(990)),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(reconstructed.after_bytes(), after);
            let fresh = plan
                .requalify_with_budgets(
                    image(&before, CatalogScope::new(991)),
                    &mut budget(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(fresh.after_bytes(), after);
            assert_eq!(fresh.identity().column_number(), 1);
            assert_eq!(fresh.identity().column_id(), 1);

            for profile in [1_u16, 5] {
                let mut invalid = encoded.clone();
                invalid[12..14].copy_from_slice(&profile.to_le_bytes());
                let mut work = budget();
                let bytes = work.remaining(Resource::Bytes);
                assert!(matches!(
                    NativeFieldRenameIntent::decode(&invalid, &mut work, &|| false),
                    Err(FieldRenameIntentError::InvalidRecord)
                ));
                assert_eq!(work.remaining(Resource::Bytes), bytes);
            }
        }
    }
}

#[test]
fn fitted_pending_and_committed_journals_reconstruct_both_images_before_recovery() {
    for replacement in ["TaxAmount", "X"] {
        let [before, after] = fitting::pair(SourceCase::NumberHistory, replacement);
        let original = fitting::old_name(SourceCase::NumberHistory);
        let wire = fitted_wire(&before, original, replacement);
        for committed in [false, true] {
            let mut target = MemoryStorage::new(before.clone());
            let mut journal = MemoryStorage::new(Vec::new());
            let prepared = fitted_plan(&before, original, replacement)
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
            }
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let recovery = qualify_native_field_rename_recovery(
                checked,
                decode(&wire),
                SCOPE,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let outcome = recovery
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                outcome,
                if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            assert_eq!(
                target.bytes.get_ref(),
                if committed { &after } else { &before }
            );
        }

        let mut forged = after;
        *forged.last_mut().unwrap() ^= 1;
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
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_field_rename_recovery(
                checked,
                decode(&wire),
                SCOPE,
                &mut budget(),
                &|| false,
            ),
            Err(FieldRenameIntentError::Proof(
                NativeRecoveryQualificationError::ImageMismatch
            ))
        ));
        assert_eq!(target.writes, 0);
    }
}
