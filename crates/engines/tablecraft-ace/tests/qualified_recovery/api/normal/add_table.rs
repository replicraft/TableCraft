//! Distinct AddTable recovery uses original populated-source and literal append oracles.
use super::{Storage, budget, resizable_storage::Target, scope};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    AddTableError, AddTableIntentError, AddTablePlan, CreateColumn, CreateDatabaseError,
    FirstTableDefinition, InspectError, NativeAddTableIntent, NativeImage, NativeIntentError,
    NativeRecoveryQualificationError, PropertyGroupKind, QualifiedAddTableRecovery,
    inspect_table_properties, qualify_native_add_table_recovery, read_table,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::journal::{RecoveryOutcome, V2State, inspect_v2, prepare_resizable_expected};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{canonical_add_table, canonical_repeated_add_table};

fn definition() -> FirstTableDefinition {
    definition_named("More")
}

fn definition_named(table: &str) -> FirstTableDefinition {
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::short_text("Name", 20).unwrap(),
    ];
    FirstTableDefinition::capture(table, &columns, &mut budget(), &|| false).unwrap()
}

fn plan(before: &[u8]) -> AddTablePlan {
    plan_named(before, "More")
}

fn plan_named(before: &[u8], table: &str) -> AddTablePlan {
    NativeImage::capture(&mut Cursor::new(before), scope(), &mut budget(), &|| false)
        .unwrap()
        .plan_add_table(definition_named(table), &mut budget(), &|| false)
        .unwrap()
}

fn intent(raw: &[u8]) -> NativeAddTableIntent {
    NativeAddTableIntent::decode(raw, &mut budget(), &|| false).unwrap()
}

fn pending(before: &[u8], after: &[u8]) -> Storage {
    let mut target = Target::new(before.to_vec());
    let mut journal = Storage::new(Vec::new());
    drop(
        prepare_resizable_expected(
            &mut target,
            &mut journal,
            before,
            after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    assert_eq!(target.storage.writes, 0);
    journal
}

fn qualify<'a>(
    journal: &'a mut Storage,
    raw: &[u8],
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedAddTableRecovery<'a, Storage>, NativeRecoveryQualificationError> {
    let checked = inspect_v2(journal, &mut budget(), &|| false).unwrap();
    qualify_native_add_table_recovery(checked, intent(raw), scope(), work, cancelled)
}

#[test]
fn add_table_commits_literal_append_and_cold_committed_recovery_keeps_original_row_and_gaps() {
    let source = canonical_add_table::source();
    let raw = plan(&source.before)
        .encode_intent(&mut budget(), &|| false)
        .unwrap();
    let mut target = Target::new(source.before.clone());
    let mut journal = Storage::new(Vec::new());
    plan(&source.before)
        .prepare(&mut target, &mut journal, &mut budget(), &|| false)
        .unwrap()
        .commit(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(target.storage.bytes.get_ref(), &source.after);
    assert_eq!(target.storage.bytes.get_ref().len(), 13 * 4096);
    let image = NativeImage::capture(
        &mut Cursor::new(&source.after),
        scope(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let original = read_table(
        &mut image.reader(),
        scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(original.rows(), &[source.original_row]);
    let added = read_table(&mut image.reader(), scope(), "More", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert!(added.rows().is_empty());

    // Cold sessions retain only original intent bytes and journal bytes; no plan
    // or candidate object grants recovery authority.
    let mut cold = Storage::new(journal.bytes.get_ref().clone());
    let qualified = qualify(&mut cold, &raw, &mut budget(), &|| false).unwrap();
    assert_eq!(qualified.state(), V2State::Committed);
    let writes = target.storage.writes;
    let resizes = target.resizes;
    assert_eq!(
        qualified
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap(),
        RecoveryOutcome::Committed
    );
    assert_eq!((target.storage.writes, target.resizes), (writes, resizes));
    assert_eq!(target.storage.bytes.get_ref(), &source.after);

    let mut cold = Storage::new(journal.bytes.get_ref().clone());
    let qualified = qualify(&mut cold, &raw, &mut budget(), &|| false).unwrap();
    assert_eq!(
        qualified
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap(),
        RecoveryOutcome::Committed
    );
    assert_eq!(target.storage.bytes.get_ref(), &source.after);
}

#[test]
fn add_table_pending_and_interrupted_growth_rollback_exact_populated_before() {
    let source = canonical_add_table::source();
    let raw = plan(&source.before)
        .encode_intent(&mut budget(), &|| false)
        .unwrap();
    for interrupted in [false, true] {
        let mut target = Target::new(source.before.clone());
        let mut journal = Storage::new(Vec::new());
        if interrupted {
            target.storage.fail_prefix = Some(45_000);
            assert!(
                plan(&source.before)
                    .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .is_err()
            );
            assert!(target.storage.bytes.get_ref().len() > source.before.len());
            assert_ne!(target.storage.bytes.get_ref(), &source.before);
        } else {
            drop(
                plan(&source.before)
                    .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap(),
            );
            assert_eq!(target.storage.writes, 0);
        }
        let mut cold = Storage::new(journal.bytes.get_ref().clone());
        let qualified = qualify(&mut cold, &raw, &mut budget(), &|| false).unwrap();
        assert_eq!(qualified.state(), V2State::Pending);
        assert_eq!(
            qualified
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap(),
            RecoveryOutcome::RolledBack
        );
        assert_eq!(target.storage.bytes.get_ref(), &source.before);
        assert_eq!(target.storage.bytes.get_ref().len(), 10 * 4096);
    }

    let mut target = Target::new(source.before.clone());
    let mut journal = Storage::new(Vec::new());
    assert_eq!(
        plan(&source.before)
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap()
            .rollback(&mut budget(), &|| false)
            .unwrap(),
        RecoveryOutcome::RolledBack
    );
    assert_eq!(target.storage.bytes.get_ref(), &source.before);
}

#[test]
fn add_table_refuses_mismatched_target_requests_source_and_complete_images_before_writes() {
    let source = canonical_add_table::source();
    let raw = plan(&source.before)
        .encode_intent(&mut budget(), &|| false)
        .unwrap();
    let mut wrong_target = source.before.clone();
    wrong_target[7 * 4096 + 3500] ^= 1;
    let mut target = Target::new(wrong_target.clone());
    let mut empty = Storage::new(Vec::new());
    assert!(
        plan(&source.before)
            .prepare(&mut target, &mut empty, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(target.storage.bytes.get_ref(), &wrong_target);
    assert_eq!(
        (target.storage.writes, target.resizes, empty.writes),
        (0, 0, 0)
    );

    for case in 0..7 {
        let mut before = source.before.clone();
        let mut after = source.after.clone();
        let mut requested = raw.clone();
        match case {
            0 => before[7 * 4096 + 3500] ^= 1,
            1 => after[3 * 4096 + 512] ^= 1,
            2 => after[4 * 4096 + 512] ^= 1,
            3 => after[8 * 4096 + 4090] ^= 1,
            4 => after[12 * 4096 + 3500] ^= 1,
            5 => *requested.last_mut().unwrap() = b'x',
            6 => {
                before = canonical_add_table::claimed_append_destination(false);
                requested =
                    NativeAddTableIntent::new(definition(), &before, &mut budget(), &|| false)
                        .unwrap()
                        .encode(&mut budget(), &|| false)
                        .unwrap();
            }
            _ => unreachable!(),
        }
        let mut journal = pending(&before, &after);
        let bytes = journal.bytes.get_ref().clone();
        let writes = journal.writes;
        let error = qualify(&mut journal, &requested, &mut budget(), &|| false)
            .err()
            .unwrap_or_else(|| panic!("tampered case {case} unexpectedly qualified"));
        let expected = match case {
            0 => matches!(
                error,
                NativeRecoveryQualificationError::AddTableIntent(
                    AddTableIntentError::BeforeMismatch
                )
            ),
            1..=5 => matches!(error, NativeRecoveryQualificationError::ImageMismatch),
            6 => matches!(
                error,
                NativeRecoveryQualificationError::AddTable(AddTableError::Source)
            ),
            _ => unreachable!(),
        };
        assert!(expected, "case {case}, typed error {error:?}");
        assert_eq!(journal.writes, writes);
        assert_eq!(journal.bytes.get_ref(), &bytes);
    }
    // A valid nested TCNT declaration is not an AddTable action and is never
    // retried by another decoder when the outer namespace fails.
    assert!(NativeAddTableIntent::decode(&raw[64..], &mut budget(), &|| false).is_err());
}

#[test]
fn add_table_qualification_charges_all_pools_and_cancellation_retains_same_journal() {
    let source = canonical_add_table::source();
    assert_qualification_controls(&source.before, &source.after, "More");
    let repeated = canonical_repeated_add_table::source();
    assert_qualification_controls(&repeated.before, &repeated.after, "Third");
}

fn assert_qualification_controls(before: &[u8], after: &[u8], table: &str) {
    let raw = plan_named(before, table)
        .encode_intent(&mut budget(), &|| false)
        .unwrap();
    let journal = pending(before, after).bytes.into_inner();
    let mut measured = budget();
    let polls = Cell::new(0_usize);
    let mut storage = Storage::new(journal.clone());
    drop(
        qualify(&mut storage, &raw, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap(),
    );
    assert_eq!(storage.writes, 0);
    assert_eq!(storage.bytes.get_ref(), &journal);
    let used = [Resource::Bytes, Resource::Rows, Resource::Steps]
        .map(|pool| budget().remaining(pool) - measured.remaining(pool));
    assert!(used.iter().all(|cost| *cost > 0));
    let mut exact = ResourceBudget::new(used[0], used[1], used[2]);
    drop(
        qualify(
            &mut Storage::new(journal.clone()),
            &raw,
            &mut exact,
            &|| false,
        )
        .unwrap(),
    );
    for dimension in 0..3 {
        let mut limits = used;
        limits[dimension] -= 1;
        let mut short = ResourceBudget::new(limits[0], limits[1], limits[2]);
        let mut storage = Storage::new(journal.clone());
        assert!(qualify(&mut storage, &raw, &mut short, &|| false).is_err());
        assert_eq!(storage.writes, 0);
        assert_eq!(storage.bytes.get_ref(), &journal);
    }
    for stop in [1, polls.get().div_ceil(2), polls.get()] {
        let seen = Cell::new(0);
        let mut storage = Storage::new(journal.clone());
        let error = qualify(&mut storage, &raw, &mut budget(), &|| {
            seen.set(seen.get() + 1);
            seen.get() >= stop
        })
        .err()
        .unwrap_or_else(|| panic!("qualification ignored cancellation at poll {stop}"));
        assert!(
            matches!(
                &error,
                NativeRecoveryQualificationError::Inspect(InspectError::Cancelled)
                    | NativeRecoveryQualificationError::AddTableIntent(
                        AddTableIntentError::Inspect(InspectError::Cancelled)
                    )
                    | NativeRecoveryQualificationError::AddTableIntent(
                        AddTableIntentError::Binding(NativeIntentError::Inspect(
                            InspectError::Cancelled
                        ))
                    )
                    | NativeRecoveryQualificationError::AddTable(AddTableError::Inspect(
                        InspectError::Cancelled
                    ))
                    | NativeRecoveryQualificationError::AddTable(AddTableError::Create(
                        CreateDatabaseError::Inspect(InspectError::Cancelled)
                    ))
            ),
            "stop {stop}, observed polls {}, typed error {error:?}",
            seen.get(),
        );
        assert_eq!(seen.get(), stop);
        assert_eq!(storage.writes, 0);
        assert_eq!(storage.bytes.get_ref(), &journal);
    }
}

#[test]
fn repeated_v2_journal_commit_and_interrupted_growth_recovery_preserve_two_original_rows() {
    let source = canonical_repeated_add_table::source();
    let raw = plan_named(&source.before, "Third")
        .encode_intent(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(&raw[..8], b"TCAT0002");
    for committed in [false, true] {
        let mut target = Target::new(source.before.clone());
        let mut journal = Storage::new(Vec::new());
        if !committed {
            target.storage.fail_prefix = Some(57_000);
        }
        let result = plan_named(&source.before, "Third")
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap()
            .commit(&mut budget(), &|| false);
        if committed {
            result.unwrap();
            assert_eq!(target.storage.bytes.get_ref(), &source.after);
        } else {
            assert!(result.is_err());
            assert!(target.storage.bytes.get_ref().len() > source.before.len());
            assert_ne!(target.storage.bytes.get_ref(), &source.before);
        }
        let mut cold = Storage::new(journal.bytes.get_ref().clone());
        let qualified = qualify(&mut cold, &raw, &mut budget(), &|| false).unwrap();
        assert_eq!(
            qualified.state(),
            if committed {
                V2State::Committed
            } else {
                V2State::Pending
            }
        );
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
        let wanted = if committed {
            &source.after
        } else {
            &source.before
        };
        assert_eq!(target.storage.bytes.get_ref(), wanted);
        assert_eq!(
            &target.storage.bytes.get_ref()[7 * 4096..13 * 4096],
            &source.before[7 * 4096..13 * 4096]
        );
        for (table, row) in ["Items", "More"].into_iter().zip(&source.original_rows) {
            let actual = read_table(
                &mut Cursor::new(target.storage.bytes.get_ref()),
                scope(),
                table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(actual.rows(), std::slice::from_ref(row));
        }
    }
}

#[test]
fn repeated_v2_recovery_refuses_altered_original_or_replacement_and_cannot_downgrade_to_v1() {
    let source = canonical_repeated_add_table::source();
    let original = plan_named(&source.before, "Third")
        .encode_intent(&mut budget(), &|| false)
        .unwrap();
    for before_changed in [false, true] {
        for at in [10 * 4096 + 3500, 11 * 4096 + 512, 12 * 4096 + 3500] {
            let mut before = source.before.clone();
            let mut after = source.after.clone();
            if before_changed {
                before[at] ^= 1;
            } else {
                after[at] ^= 1;
            }
            let mut journal = pending(&before, &after);
            let bytes = journal.bytes.get_ref().clone();
            let writes = journal.writes;
            let error = qualify(&mut journal, &original, &mut budget(), &|| false)
                .err()
                .expect("tampered repeated images must refuse");
            if before_changed {
                assert!(matches!(
                    error,
                    NativeRecoveryQualificationError::AddTableIntent(
                        AddTableIntentError::BeforeMismatch
                    )
                ));
            } else {
                assert!(matches!(
                    error,
                    NativeRecoveryQualificationError::ImageMismatch
                ));
            }
            assert_eq!(journal.writes, writes);
            assert_eq!(journal.bytes.get_ref(), &bytes);
        }
    }
    let mut downgraded = original;
    downgraded[..8].copy_from_slice(b"TCAT0001");
    downgraded[12..14].copy_from_slice(&1_u16.to_le_bytes());
    assert!(matches!(
        NativeAddTableIntent::decode(&downgraded, &mut budget(), &|| false),
        Err(AddTableIntentError::InvalidRecord)
    ));
}

#[test]
fn described_append_actions_recover_only_exact_tcnt5_requests_in_both_source_partitions() {
    for repeated in [false, true] {
        let before = if repeated {
            canonical_repeated_add_table::source().before
        } else {
            canonical_add_table::source().before
        };
        let table = if repeated { "Third" } else { "More" };
        let columns = [
            CreateColumn::new("Id", ScalarType::Int32)
                .with_primary_key("PrimaryId")
                .unwrap()
                .with_incrementing_autonumber()
                .unwrap()
                .with_description("")
                .unwrap(),
            CreateColumn::short_text("Name", 20)
                .unwrap()
                .with_description("Résumé 🗃 help")
                .unwrap(),
        ];
        let definition =
            FirstTableDefinition::capture(table, &columns, &mut budget(), &|| false).unwrap();
        let plan =
            NativeImage::capture(&mut Cursor::new(&before), scope(), &mut budget(), &|| false)
                .unwrap()
                .plan_add_table(definition, &mut budget(), &|| false)
                .unwrap();
        let raw = plan.encode_intent(&mut budget(), &|| false).unwrap();
        assert_eq!(&raw[..8], if repeated { b"TCAT0004" } else { b"TCAT0003" });
        let after = plan.after_bytes().to_vec();
        assert_eq!(&after[7 * 4096..before.len()], &before[7 * 4096..]);
        let properties = inspect_table_properties(
            &mut Cursor::new(&after),
            scope(),
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for (field, wanted) in [("Id", ""), ("Name", "Résumé 🗃 help")] {
            let actual = properties
                .group(PropertyGroupKind::Column, field)
                .unwrap()
                .get("Description")
                .unwrap();
            assert_eq!(actual.native_type(), 10);
            assert_eq!(actual.text().unwrap(), wanted);
        }
        for committed in [false, true] {
            let mut target = Target::new(before.clone());
            let mut journal = pending(&before, &after);
            if committed {
                let original_plan = NativeImage::capture(
                    &mut Cursor::new(&before),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .plan_add_table(
                    FirstTableDefinition::capture(table, &columns, &mut budget(), &|| false)
                        .unwrap(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                // A real commit, not a forged marker, establishes the cold state.
                journal = Storage::new(Vec::new());
                original_plan
                    .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap();
            }
            let mut cold = Storage::new(journal.bytes.get_ref().clone());
            let checked = qualify(&mut cold, &raw, &mut budget(), &|| false).unwrap();
            assert_eq!(
                checked
                    .recover(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            assert_eq!(
                target.storage.bytes.get_ref(),
                if committed { &after } else { &before }
            );
        }
        let mut changed = raw.clone();
        *changed.last_mut().unwrap() = b'x';
        let mut journal = pending(&before, &after);
        let writes = journal.writes;
        let error = qualify(&mut journal, &changed, &mut budget(), &|| false)
            .err()
            .expect("changed description cannot authorize unchanged AFTER");
        assert!(matches!(
            error,
            NativeRecoveryQualificationError::ImageMismatch
        ));
        assert_eq!(journal.writes, writes);
    }
}

#[path = "add_table/mixed.rs"]
mod mixed;
