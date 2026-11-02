//! Future child of qualified_recovery::api::normal; no runtime result claimed.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use std::io::Cursor;
use tablecraft_ace::{
    FixedScalarUpdate, NativeFixedIntentGroup, NativeImage, NativeUpdateIntent,
    qualify_native_update_recovery,
};
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::canonical_fixed_before_change::{self as model, Case};

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn groups(case: &Case) -> Vec<NativeFixedIntentGroup> {
    case.groups
        .iter()
        .map(|group| {
            let field = model::FIELDS
                .iter()
                .find(|field| field.0 == group.column)
                .unwrap();
            let updates = group
                .updates
                .iter()
                .map(|update| {
                    FixedScalarUpdate::new(
                        update.ordinal,
                        update.expected.value(),
                        update.replacement.value(),
                    )
                })
                .collect();
            NativeFixedIntentGroup::new(field.1, updates, &mut budget(), &|| false).unwrap()
        })
        .collect()
}

fn original(intent: &NativeUpdateIntent, case: &Case) {
    assert_eq!(intent.table(), "Items");
    assert!(intent.int32().is_none());
    let fixed = intent.fixed().expect("typed original fixed requests");
    assert_eq!(fixed.groups().len(), case.groups.len());
    for (actual, expected) in fixed.groups().iter().zip(&case.groups) {
        let field = model::FIELDS
            .iter()
            .find(|field| field.0 == expected.column)
            .unwrap();
        assert_eq!(actual.column(), field.1);
        assert_eq!(actual.updates().len(), expected.updates.len());
        for (actual, expected) in actual.updates().iter().zip(&expected.updates) {
            assert_eq!(actual.row(), expected.ordinal);
            assert_eq!(actual.expected(), &expected.expected.value());
            assert_eq!(actual.value(), &expected.replacement.value());
        }
    }
}

#[test]
fn typed_fixed_normal_commits_twenty_eight_literal_images_with_original_groups() {
    let mut committed = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let Some(expected) = &case.expected else {
                continue;
            };
            let plan = image(&case.before)
                .plan_recoverable_fixed_updates("Items", groups(&case), &mut budget(), &|| false)
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), expected);
            assert_eq!(plan.changed_rows(), case.changed_rows);
            original(plan.intent(), &case);
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(&wire[..8], b"TCNI0003");
            let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            original(&decoded, &case);

            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            let prepared = plan
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                prepared.commit(&mut budget(), &|| false).unwrap(),
                case.changed_rows
            );
            assert_eq!(target.bytes.get_ref(), expected);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, expected, Marker::Committed)
            );
            committed += 1;
        }
    }
    assert_eq!(committed, 28);
}

#[test]
fn typed_fixed_recovery_replays_original_groups_in_all_journal_states() {
    let mut cases = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let Some(expected) = &case.expected else {
                continue;
            };
            let plan = image(&case.before)
                .plan_recoverable_fixed_updates("Items", groups(&case), &mut budget(), &|| false)
                .unwrap();
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            original(plan.intent(), &case);

            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let mut journal = Storage::new(journals::journal(&case.before, expected, marker));
                let saved = journal.bytes.get_ref().clone();
                let mut target = Storage::new(expected.clone());
                for _ in 0..2 {
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let intent =
                        NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
                    original(&intent, &case);
                    let qualified = qualify_native_update_recovery(
                        checked,
                        intent,
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
                        if committed { expected } else { &case.before }
                    );
                }
                if committed {
                    assert_eq!(target.writes, 0);
                }
                let writes = target.writes;
                target.fail_prefix = Some(0);
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let intent = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_update_recovery(
                    checked,
                    intent,
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
                assert_eq!(journal.bytes.get_ref(), &saved);
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 28);
}
