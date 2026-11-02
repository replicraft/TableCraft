//! Cold replay keeps original omissions and uses the same checked V1 journal.
use super::super::{Storage, budget, model as journals, scope};
use super::{cases, wire};
use journals::Marker;
use tablecraft_ace::{NativeOmittedInsertIntent, qualify_native_omitted_insert_recovery};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};

#[test]
#[ignore = "requires independently qualified companion sources; cold TCNA2 replay"]
fn full_and_causally_mixed_increment_targets_recover_repeat_and_confirm() {
    let cases = cases();
    let mut contexts = 0;
    for case in &cases {
        let before = case.before.as_slice();
        let after = case.after.as_slice();
        let mut mixed = before.to_vec();
        // LAST, TD live count and primary history changed, while DATA and
        // the leaf population are still original: a genuine partial state.
        let definition = case.definition as usize * 4096;
        mixed[definition..definition + 4096].copy_from_slice(&after[definition..definition + 4096]);
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
                        &wire(case, false),
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
                let intent =
                    NativeOmittedInsertIntent::decode(&wire(case, false), &mut budget(), &|| false)
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
    assert_eq!(contexts, 120);
}

#[test]
#[ignore = "requires original/AFTER pins; no producer or encoder supplies journal images"]
fn protected_source_bytes_and_generated_counter_association_refuse_before_effects() {
    let cases = cases();
    let mut refused = 0;

    for case in &cases {
        for offset in case.protected_offsets() {
            let mut forged = case.after.clone();
            forged[offset] ^= 1;
            let mut journal =
                Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let intent =
                NativeOmittedInsertIntent::decode(&wire(case, false), &mut budget(), &|| false)
                    .unwrap();

            assert!(
                qualify_native_omitted_insert_recovery(
                    checked,
                    intent,
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .is_err()
            );
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
            refused += 1;
        }
    }

    // Eight protected roles per source, plus six inactive payloads and the two
    // genuine deleted-high physical slots. These are original-source offsets.
    assert_eq!(refused, 168);
}
