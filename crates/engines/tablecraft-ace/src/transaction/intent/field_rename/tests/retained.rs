//! Independent retained-history request bytes and source-bound cold recovery.
use super::*;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        self as history, readdition::Target,
    },
    saved_field_rename::fitting::current_history,
};

fn retained_plan(before: &[u8], old: &str, new: &str) -> RecoverableFieldRenamePlan {
    image(before, SCOPE)
        .plan_recoverable_retained_field_rename("Table1", old, new, &mut budget(), &|| false)
        .unwrap()
}

fn retained_literal(case: history::Case, before: &[u8], old: &str, new: &str) -> Vec<u8> {
    let mut wire = literal(before, old, new);
    let dropped = history::identity(case);
    wire[12..14].copy_from_slice(&5_u16.to_le_bytes());
    wire[64..66].copy_from_slice(&(dropped.native_number + 1).to_le_bytes());
    wire[66..68].copy_from_slice(&dropped.column_id.to_le_bytes());
    wire
}

#[test]
fn retained_rename_wire_preserves_distinct_native_identity_and_closed_older_languages() {
    for case in history::CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                let before = current_history::source(case, target, filled);
                for replacement in ["RenamedField", "X", target.name(), "nextfield"] {
                    let expected = current_history::rename(&before, target.name(), replacement);
                    let reviewed = retained_plan(&before, target.name(), replacement);
                    let wire = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
                    assert_eq!(
                        wire,
                        retained_literal(case, &before, target.name(), replacement)
                    );
                    assert_eq!(
                        decode(&wire).encode(&mut budget(), &|| false).unwrap(),
                        wire
                    );
                    assert_ne!(
                        reviewed.identity().column_number(),
                        usize::from(reviewed.identity().column_id())
                    );
                    let fresh = reviewed
                        .requalify_with_budgets(
                            image(&before, CatalogScope::new(1781)),
                            &mut budget(),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                    assert_eq!(fresh.after_bytes(), expected);
                    assert_eq!(fresh.changed(), target.name() != replacement);
                    for tag in [1_u16, 2, 3, 4] {
                        let mut legacy = wire.to_vec();
                        legacy[12..14].copy_from_slice(&tag.to_le_bytes());
                        let reconstructed =
                            NativeFieldRenameIntent::decode(&legacy, &mut budget(), &|| false)
                                .and_then(|request| {
                                    request.reconstruct(
                                        image(&before, SCOPE),
                                        &mut budget(),
                                        &|| false,
                                    )
                                });
                        assert!(
                            reconstructed.is_err(),
                            "profile {tag} must retain its original source"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn retained_rename_cold_recovery_reconstructs_original_request_for_pending_and_committed_states() {
    for case in history::CASES {
        for target in [Target::Number, Target::ShortText] {
            let before = current_history::source(case, target, true);
            let after = current_history::rename(&before, target.name(), "RenamedField");
            let wire = retained_literal(case, &before, target.name(), "RenamedField");
            for committed in [false, true] {
                let mut target_file = MemoryStorage::new(before.to_vec());
                let mut journal_file = MemoryStorage::new(Vec::new());
                let prepared = retained_plan(&before, target.name(), "RenamedField")
                    .prepare(&mut target_file, &mut journal_file, &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                if committed {
                    prepared.commit(&mut budget(), &|| false).unwrap();
                } else {
                    drop(prepared);
                    target_file.bytes.get_mut().copy_from_slice(&after);
                }
                let checked = inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
                qualify_native_field_rename_recovery(
                    checked,
                    decode(&wire),
                    CatalogScope::new(1782),
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .recover(&mut target_file, &mut budget(), &|| false)
                .unwrap();
                assert_eq!(
                    target_file.bytes.get_ref(),
                    if committed { &after } else { &before }
                );
            }
        }
    }
}

#[test]
fn retained_rename_valid_journal_digests_do_not_authorize_unrelated_after_changes() {
    let case = history::Case::MixedBinary;
    let before = current_history::source(case, Target::ShortText, true);
    let after = current_history::rename(&before, "NextText", "RenamedField");
    let wire = retained_literal(case, &before, "NextText", "RenamedField");
    for offset in [
        7 * 4096 + 1000,
        8 * 4096 + 4090,
        9 * 4096 + 482,
        10 * 4096 + 4090,
    ] {
        let mut forged = after.to_vec();
        forged[offset] ^= 1;
        let mut target_file = MemoryStorage::new(before.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_expected(
                &mut target_file,
                &mut journal_file,
                &before,
                &forged,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
        assert!(
            qualify_native_field_rename_recovery(
                checked,
                decode(&wire),
                SCOPE,
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
        assert_eq!(target_file.writes, 0);
        assert_eq!(target_file.bytes.get_ref(), &before);
    }
}
