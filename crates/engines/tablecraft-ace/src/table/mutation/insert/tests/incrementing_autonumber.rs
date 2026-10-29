//! Caller-selected increment policy, original counters and independent whole images.
pub(in crate::table::mutation::insert) mod model;

use super::model as original;
use crate::{
    AutoNumberRowInsert, EditError, IncrementingAutoNumber, IndexConstraintViolation, InspectError,
    RowInsert, RuleCheckStatus, check_table_constraints, read_table,
};
use model::{Row, State};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn additions(state: State) -> Vec<Row> {
    let ids: &[i32] = match state {
        State::Fresh => &[1, 2],
        State::Negative => &[-1, 0],
        State::Minimum => &[i32::MIN + 1, i32::MIN + 2],
        State::HighestDeleted | State::AllDeleted => &[4, 5],
        State::HighWater => &[261, 262],
        State::NearMaximum => &[i32::MAX],
        State::Pressure => &[601, 602, 603, 604],
        _ => &[],
    };
    ids.iter()
        .enumerate()
        .map(|(position, &id)| Row {
            earlier: -100 - position as i32,
            id,
            score: if matches!(state, State::Pressure) {
                [Some(20), Some(20), Some(30), None][position]
            } else if position == 0 {
                Some(20)
            } else {
                None
            },
        })
        .collect()
}

fn requests(rows: &[Row]) -> Vec<AutoNumberRowInsert> {
    rows.iter()
        .map(|row| AutoNumberRowInsert::new(row.companions()))
        .collect()
}

fn checked(bytes: &[u8], rows: &[Row]) {
    let report = original::checked(bytes, rows.len(), 4);
    let primary = report
        .indexes()
        .iter()
        .find(|index| {
            index
                .index()
                .constraints()
                .is_some_and(|rules| rules.primary_key())
        })
        .unwrap();
    assert_eq!(primary.index().columns()[0].number(), 1);
    let snapshot = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        snapshot.rows(),
        rows.iter().map(Row::values).collect::<Vec<_>>()
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(constraints.table_validation(), RuleCheckStatus::Unsupported);
    for field in constraints.fields() {
        assert!(
            ![field.required(), field.empty_text(), field.validation()]
                .contains(&RuleCheckStatus::Unsupported)
        );
    }
}

#[test]
fn qualify_incrementing_autonumber_sources_and_whole_expected_images_before_preview() {
    for code in [5, 6] {
        for state in [
            State::Fresh,
            State::Negative,
            State::Minimum,
            State::HighestDeleted,
            State::AllDeleted,
            State::HighWater,
            State::NearMaximum,
            State::Pressure,
        ] {
            let case = model::source(code, state);
            let added = additions(state);
            let expected = model::expected(&case, &added);
            let mut rows = model::live(&case);
            checked(&case.before, &rows);
            rows.extend(added);
            checked(&expected, &rows);
            assert_eq!(
                &expected[model::DEFINITION + 28..model::DEFINITION + 32],
                &case.before[model::DEFINITION + 28..model::DEFINITION + 32]
            );
            if matches!(state, State::Pressure) {
                assert_eq!(case.before.len() / model::PAGE, 18);
                assert_eq!(expected.len() / model::PAGE, 22);
                assert_eq!(&expected[13 * model::PAGE..13 * model::PAGE + 2], &[3, 1]);
                assert_eq!(&expected[14 * model::PAGE..14 * model::PAGE + 2], &[3, 1]);
            }
        }
        for state in [State::Maximum, State::Collision] {
            let case = model::source(code, state);
            checked(&case.before, &model::live(&case));
        }
    }
}

#[test]
fn generated_primary_uses_native_column_one_and_persisted_high_water() {
    for code in [5, 6] {
        for state in [
            State::Fresh,
            State::Negative,
            State::Minimum,
            State::HighestDeleted,
            State::AllDeleted,
            State::HighWater,
        ] {
            let case = model::source(code, state);
            let added = additions(state);
            let expected = model::expected(&case, &added);
            let mut rows = model::live(&case);
            checked(&case.before, &rows);
            rows.extend(added.iter().cloned());
            checked(&expected, &rows);
            let candidate = original::capture(&case.before)
                .preview_ace16_incrementing_insert_rows(
                    "Items",
                    &requests(&added),
                    IncrementingAutoNumber::new(case.last),
                    &mut original::budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(candidate.before_bytes(), case.before);
            assert_eq!(candidate.candidate_bytes(), expected);
            assert_eq!(candidate.generated_column_number(), 1);
            assert_eq!(
                candidate.generated_values(),
                added.iter().map(|row| row.id).collect::<Vec<_>>()
            );
            assert_eq!(candidate.inserted_rows(), added.len());
        }
    }
}

#[test]
fn generated_requests_share_data_and_both_physical_split_allocations() {
    for code in [5, 6] {
        let case = model::source(code, State::Pressure);
        let added = additions(State::Pressure);
        let expected = model::expected(&case, &added);
        let mut rows = model::live(&case);
        checked(&case.before, &rows);
        rows.extend(added.iter().cloned());
        checked(&expected, &rows);
        let candidate = original::capture(&case.before)
            .preview_ace16_incrementing_insert_rows(
                "Items",
                &requests(&added),
                IncrementingAutoNumber::new(600),
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(candidate.candidate_bytes(), expected);
        assert_eq!(candidate.generated_values(), &[601, 602, 603, 604]);
        assert_eq!(candidate.inserted_rows(), 4);
    }
}

#[test]
fn checked_positive_exhaustion_and_empty_maximum_never_wrap_or_reseed() {
    for code in [5, 6] {
        let case = model::source(code, State::NearMaximum);
        let added = additions(State::NearMaximum);
        let expected = model::expected(&case, &added);
        checked(&case.before, &[]);
        checked(&expected, &added);
        let candidate = original::capture(&case.before)
            .preview_ace16_incrementing_insert_rows(
                "Items",
                &requests(&added),
                IncrementingAutoNumber::new(i32::MAX - 1),
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(candidate.candidate_bytes(), expected);
        assert_eq!(candidate.generated_values(), &[i32::MAX]);
        let over = [
            AutoNumberRowInsert::new(vec![Value::Int32(1), Value::Null]),
            AutoNumberRowInsert::new(vec![Value::Int32(2), Value::Null]),
        ];
        assert_eq!(
            original::capture(&case.before)
                .preview_ace16_incrementing_insert_rows(
                    "Items",
                    &over,
                    IncrementingAutoNumber::new(i32::MAX - 1),
                    &mut original::budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::AutoNumberExhausted)
        );
        let case = model::source(code, State::Maximum);
        let empty = original::capture(&case.before)
            .preview_ace16_incrementing_insert_rows(
                "Items",
                &[],
                IncrementingAutoNumber::new(i32::MAX),
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(empty.candidate_bytes(), case.before);
        assert!(empty.generated_values().is_empty());
        assert_eq!(
            original::capture(&case.before)
                .preview_ace16_incrementing_insert_rows(
                    "Items",
                    &requests(&added),
                    IncrementingAutoNumber::new(i32::MAX),
                    &mut original::budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::AutoNumberExhausted)
        );
    }
}

#[test]
fn stale_counter_collisions_companions_and_manual_entry_do_not_bypass_authority() {
    let case = model::source(6, State::HighestDeleted);
    checked(&case.before, &model::live(&case));
    let added = additions(State::HighestDeleted);
    assert_eq!(
        original::capture(&case.before)
            .preview_ace16_incrementing_insert_rows(
                "Items",
                &requests(&added),
                IncrementingAutoNumber::new(2),
                &mut original::budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Conflict)
    );
    for values in [
        vec![],
        vec![Value::Int32(1)],
        vec![Value::Int32(1), Value::Int32(4), Value::Null],
        vec![Value::Int64(1), Value::Null],
    ] {
        assert!(
            original::capture(&case.before)
                .preview_ace16_incrementing_insert_rows(
                    "Items",
                    &[AutoNumberRowInsert::new(values)],
                    IncrementingAutoNumber::new(3),
                    &mut original::budget(),
                    &|| false,
                )
                .is_err()
        );
    }
    assert!(
        original::capture(&case.before)
            .preview_ace16_insert_rows(
                "Items",
                &[RowInsert::new(added[0].values())],
                &mut original::budget(),
                &|| false,
            )
            .is_err()
    );
    let collision = model::source(6, State::Collision);
    checked(&collision.before, &model::live(&collision));
    assert_eq!(
        original::capture(&collision.before)
            .preview_ace16_incrementing_insert_rows(
                "Items",
                &[AutoNumberRowInsert::new(vec![
                    Value::Int32(77),
                    Value::Null
                ])],
                IncrementingAutoNumber::new(3),
                &mut original::budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey
        )))
    );
}

#[test]
fn generated_preview_accounts_for_all_pools_and_terminal_cancellation() {
    let case = model::source(6, State::HighestDeleted);
    let added = additions(State::HighestDeleted);
    let expected = model::expected(&case, &added);
    let mut rows = model::live(&case);
    rows.extend(added.iter().cloned());
    checked(&expected, &rows);
    let requests = requests(&added);
    for mut budget in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            original::capture(&case.before)
                .preview_ace16_incrementing_insert_rows(
                    "Items",
                    &requests,
                    IncrementingAutoNumber::new(3),
                    &mut budget,
                    &|| false,
                )
                .is_err()
        );
    }
    let polls = Cell::new(0);
    let candidate = original::capture(&case.before)
        .preview_ace16_incrementing_insert_rows(
            "Items",
            &requests,
            IncrementingAutoNumber::new(3),
            &mut original::budget(),
            &|| {
                polls.set(polls.get() + 1);
                false
            },
        )
        .unwrap();
    assert_eq!(candidate.candidate_bytes(), expected);
    let terminal = polls.get();
    assert!(terminal > 1);
    let polls = Cell::new(0);
    assert!(
        original::capture(&case.before)
            .preview_ace16_incrementing_insert_rows(
                "Items",
                &requests,
                IncrementingAutoNumber::new(3),
                &mut original::budget(),
                &|| {
                    polls.set(polls.get() + 1);
                    polls.get() >= terminal
                },
            )
            .is_err()
    );
    assert_eq!(polls.get(), terminal);
}

#[test]
fn empty_and_reentered_candidates_bind_their_own_persisted_counter() {
    let case = model::source(6, State::Fresh);
    let first_rows = additions(State::Fresh);
    let next = model::successor(&case, &first_rows);
    let second_rows = vec![Row {
        earlier: 99,
        id: 3,
        score: Some(30),
    }];
    let second_expected = model::expected(&next, &second_rows);
    let mut all = model::live(&next);
    checked(&next.before, &all);
    all.extend(second_rows.iter().cloned());
    checked(&second_expected, &all);
    let empty = original::capture(&case.before)
        .preview_ace16_incrementing_insert_rows(
            "Items",
            &[],
            IncrementingAutoNumber::new(0),
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(empty.candidate_bytes(), case.before);
    assert!(empty.generated_values().is_empty());
    let first = original::capture(&case.before)
        .preview_ace16_incrementing_insert_rows(
            "Items",
            &requests(&first_rows),
            IncrementingAutoNumber::new(0),
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(first.candidate_bytes(), next.before);
    let second = original::capture(first.candidate_bytes())
        .preview_ace16_incrementing_insert_rows(
            "Items",
            &requests(&second_rows),
            IncrementingAutoNumber::new(2),
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(second.generated_values(), &[3]);
    assert_eq!(second.candidate_bytes(), second_expected);
}

#[test]
fn unqualified_raw_generated_flags_do_not_infer_an_increment_mode() {
    let case = model::source(6, State::Fresh);
    for flags in [0x47, 0x87, 0x0f] {
        let mut before = case.before.clone();
        before[model::GENERATED_FLAGS] = flags;
        // Deliberately unqualified metadata is a refusal control, not a claim
        // that any one of these bytes represents native Random AutoNumber.
        assert!(
            original::capture(&before)
                .preview_ace16_incrementing_insert_rows(
                    "Items",
                    &[],
                    IncrementingAutoNumber::new(0),
                    &mut original::budget(),
                    &|| false,
                )
                .is_err()
        );
    }
    let mut multiple = case.before.clone();
    multiple[model::DEFINITION + 87 + 15] = 7;
    assert!(
        original::capture(&multiple)
            .preview_ace16_incrementing_insert_rows(
                "Items",
                &[],
                IncrementingAutoNumber::new(0),
                &mut original::budget(),
                &|| false,
            )
            .is_err()
    );
}
