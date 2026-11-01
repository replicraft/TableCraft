//! Existing normal recovery child: original Id request plus macro-only Score.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use std::io::Cursor;
use tablecraft_ace::{
    EditError, Int32Update, NativeImage, NativeInt32EditIntent, NativeUpdateIntent,
    NativeUpdateProfile, RecoverableEditError,
};
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_testkit::access::canonical_nullable_index_id_before_change as model;

fn requests(case: &model::Case) -> Vec<Int32Update> {
    vec![Int32Update::new(
        case.update.ordinal,
        case.update.expected,
        case.update.replacement,
    )]
}

fn original(case: &model::Case) -> NativeUpdateIntent {
    let record = NativeInt32EditIntent::new(
        "Items",
        "Id",
        requests(case),
        &case.before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut wire = record.encode(&mut budget(), &|| false).unwrap();
    wire[..8].copy_from_slice(b"TCNI0002");
    wire[12..14].copy_from_slice(&2_u16.to_le_bytes());
    NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap()
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

#[test]
fn qualified_present_id_commits_two_companion_images_with_original_id_intent_only() {
    for header in [5, 6] {
        let case = model::source(header);
        let plan = image(&case.before)
            .plan_recoverable_int32_updates("Items", "Id", requests(&case), &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.expected);
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16BeforeChangeInt32
        );
        assert_eq!(plan.intent().column(), "Id");
        assert_eq!(plan.intent().updates().len(), 1);
        let request = &plan.intent().updates()[0];
        assert_eq!(
            (request.row(), request.expected(), request.value()),
            (0, 1, 7)
        );
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            original(&case).encode(&mut budget(), &|| false).unwrap()
        );
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 1);
        assert_eq!(target.bytes.get_ref(), &case.expected);
        assert_eq!(
            journal.bytes.get_ref(),
            &journals::journal(&case.before, &case.expected, Marker::Committed)
        );
        // A present indexed target does not make inactive NULL storage a valid
        // original expected value. No NULL→present request DTO is introduced.
        assert!(matches!(
            image(&case.before).plan_recoverable_int32_updates(
                "Items",
                "Id",
                vec![Int32Update::new(2, 3, 7)],
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::Conflict))
        ));
    }
}

#[test]
fn companion_recovery_replays_original_id_and_macro_score_in_each_journal_state() {
    for header in [5, 6] {
        let case = model::source(header);
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            let committed = matches!(marker, Marker::Committed);
            let mut journal = Storage::new(journals::journal(&case.before, &case.expected, marker));
            let saved = journal.bytes.get_ref().clone();
            let mut target = Storage::new(case.expected.clone());
            for _ in 0..2 {
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = tablecraft_ace::qualify_native_update_recovery(
                    checked,
                    original(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    qualified
                        .recover(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    }
                );
                assert_eq!(
                    target.bytes.get_ref(),
                    if committed {
                        &case.expected
                    } else {
                        &case.before
                    }
                );
            }
            if committed {
                assert_eq!(target.writes, 0);
            }
            let writes = target.writes;
            target.fail_prefix = Some(0);
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = tablecraft_ace::qualify_native_update_recovery(
                checked,
                original(&case),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                qualified
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            assert_eq!(target.writes, writes);
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}
