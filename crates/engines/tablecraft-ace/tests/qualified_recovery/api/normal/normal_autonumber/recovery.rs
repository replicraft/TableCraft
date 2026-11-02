//! Cold replay keeps original omissions and uses the same checked V1 journal.
use super::super::{Storage, budget, model as journals, scope};
use super::{fixtures, sources, wire};
use journals::Marker;
use tablecraft_ace::{
    NativeOmittedInsertIntent, NativeRecoveryQualificationError,
    qualify_native_omitted_insert_recovery,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};

#[test]
#[ignore = "requires source14; native Increment replay before journal effects"]
fn full_and_causally_mixed_increment_targets_recover_repeat_and_confirm() {
    let sources = sources();
    let mut contexts = 0;
    for header in [5, 6] {
        for recipe in fixtures::RECIPES {
            let before = sources.bytes(header, recipe.before());
            let after = sources.bytes(header, recipe.after());
            let mut mixed = before.to_vec();
            // LAST, TD live count and primary history changed, while DATA and
            // the leaf population are still original: a genuine partial state.
            mixed[7 * 4096..8 * 4096].copy_from_slice(&after[7 * 4096..8 * 4096]);
            assert_ne!(mixed, before);
            assert_ne!(mixed, after);
            for partial in [false, true] {
                for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                    let committed = matches!(marker, Marker::Committed);
                    let initial = if partial { mixed.as_slice() } else { after };
                    let expected = if committed { initial } else { before };
                    let outcome = if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    };
                    let mut target = Storage::new(initial.to_vec());
                    let mut journal = Storage::new(journals::journal(before, after, marker));
                    let saved = journal.bytes.get_ref().clone();
                    for _ in 0..2 {
                        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                        let intent = NativeOmittedInsertIntent::decode(
                            &wire(before, false),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                        let qualified = qualify_native_omitted_insert_recovery(
                            checked,
                            intent,
                            scope(),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                        let result = qualified.recover(&mut target, &mut budget(), &|| false);
                        if partial && committed {
                            assert!(matches!(result, Err(JournalError::TargetMismatch)));
                            assert_eq!(target.writes, 0);
                        } else {
                            assert_eq!(result.unwrap(), outcome);
                        }
                        assert_eq!(target.bytes.get_ref().as_slice(), expected);
                        assert_eq!(journal.bytes.get_ref(), &saved);
                        assert_eq!(journal.writes, 0);
                    }
                    let writes = target.writes;
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let intent = NativeOmittedInsertIntent::decode(
                        &wire(before, false),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let qualified = qualify_native_omitted_insert_recovery(
                        checked,
                        intent,
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let result = qualified.confirm_completed(&mut target, &mut budget(), &|| false);
                    if partial && committed {
                        assert!(matches!(result, Err(JournalError::TargetMismatch)));
                    } else {
                        assert_eq!(result.unwrap(), outcome);
                    }
                    assert_eq!(target.writes, writes);
                    assert_eq!(target.bytes.get_ref().as_slice(), expected);
                    assert_eq!(journal.writes, 0);
                    assert_eq!(journal.bytes.get_ref(), &saved);
                    contexts += 1;
                }
            }
        }
    }
    assert_eq!(contexts, 24);
}

#[test]
#[ignore = "requires source14; whole-image counter/record/index intent must agree"]
fn self_consistent_journal_forgeries_cannot_replace_original_generated_intent() {
    let sources = sources();
    let mut refused = 0;
    for header in [5, 6] {
        for recipe in fixtures::RECIPES {
            let before = sources.bytes(header, recipe.before());
            let after = sources.bytes(header, recipe.after());
            let new_start = match recipe {
                fixtures::Recipe::High => 4074,
                fixtures::Recipe::Zero => 4085,
            };
            for offset in [
                7 * 4096 + 20,
                7 * 4096 + 67,
                8 * 4096 + new_start + 6,
                9 * 4096 + 480,
                9 * 4096 + 4095,
                3 * 4096 + 4080,
            ] {
                let mut forged = after.to_vec();
                forged[offset] ^= 1;
                let mut journal = Storage::new(journals::journal(before, &forged, Marker::Pending));
                let saved = journal.bytes.get_ref().clone();
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let intent =
                    NativeOmittedInsertIntent::decode(&wire(before, false), &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                assert!(matches!(
                    qualify_native_omitted_insert_recovery(
                        checked,
                        intent,
                        scope(),
                        &mut budget(),
                        &|| false
                    ),
                    Err(NativeRecoveryQualificationError::ImageMismatch)
                ));
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
                refused += 1;
            }
        }
    }
    assert_eq!(refused, 24);
}
