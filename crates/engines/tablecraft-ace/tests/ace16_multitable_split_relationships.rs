#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "ace16_restrict_delete_contract/model.rs"]
mod model;

#[path = "ace16_multitable_split_relationships/source.rs"]
mod qualified;

use std::io::Cursor;
use tablecraft_ace::{
    Ace16WritePreview, ConstraintViolation, EditError, FixedColumnUpdates, FixedScalarUpdate,
    IndexCheckStatus, InspectError, check_table, check_table_constraints, read_table,
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

fn update(row: usize, before: i32, after: i32) -> FixedScalarUpdate {
    FixedScalarUpdate::new(row, Value::Int32(before), Value::Int32(after))
}

fn run(
    source: &model::Model,
    groups: &[FixedColumnUpdates<'_>],
) -> Result<Ace16WritePreview, EditError> {
    model::capture(&source.bytes).preview_ace16_split_batch_updates(
        groups,
        &mut model::budget(),
        &|| false,
    )
}

fn verify_rows(bytes: &[u8], expected: &[Vec<Vec<Value>>; 3]) {
    for (table, rows) in expected.iter().enumerate() {
        let checked = check_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            model::table(table),
            &mut model::budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );

        check_table_constraints(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            model::table(table),
            &mut model::budget(),
            &|| false,
        )
        .unwrap();

        let actual = read_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            model::table(table),
            &mut model::budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(actual.rows(), rows);
    }
}

#[test]
fn original_relationship_sources_pass_before_any_allocating_batch_comparison() {
    for code in [5, 6] {
        let source = qualified::source(code);
        verify_rows(&source.bytes, &source.rows);
        assert_eq!(source.bytes.len(), 25 * 4096);
        assert_eq!(source.rows.iter().map(Vec::len).sum::<usize>(), 8);
    }
}

#[test]
fn explicit_parent_and_child_validate_only_the_complete_final_image() {
    for code in [5, 6] {
        let source = qualified::source(code);
        let parent = [update(0, 10, 11)];
        let children = [update(0, 10, 11), update(1, 10, 11)];
        let mut expected = source.rows.clone();
        expected[1][0][1] = Value::Int32(11);
        expected[0][0][1] = Value::Int32(11);
        expected[0][1][1] = Value::Int32(11);

        // Either table by itself would violate the original association.
        for group in [
            FixedColumnUpdates::new("Parents", "Score", &parent),
            FixedColumnUpdates::new("Items", "Score", &children),
        ] {
            assert_eq!(
                run(&source, &[group]).err(),
                Some(EditError::Inspect(InspectError::Constraint(
                    ConstraintViolation::MissingParent
                )))
            );
        }

        let groups = [
            FixedColumnUpdates::new("Parents", "Score", &parent),
            FixedColumnUpdates::new("Items", "Score", &children),
        ];
        let candidate = run(&source, &groups).unwrap();
        assert_eq!(candidate.before_bytes(), source.bytes);
        assert_eq!(candidate.changed_rows(), 3);
        assert_eq!(candidate.candidate_bytes().len(), source.bytes.len());
        verify_rows(candidate.candidate_bytes(), &expected);

        let reversed = run(
            &source,
            &[
                FixedColumnUpdates::new("Items", "Score", &children),
                FixedColumnUpdates::new("Parents", "Score", &parent),
            ],
        )
        .unwrap();
        assert_eq!(candidate.candidate_bytes(), reversed.candidate_bytes());
    }
}

#[test]
fn an_untouched_incoming_child_blocks_an_otherwise_valid_participant_batch() {
    for code in [5, 6] {
        let source = qualified::source(code);
        let parents = [update(0, 10, 11), update(1, 20, 21)];
        let children = [update(0, 10, 11), update(1, 10, 11)];
        let groups = [
            FixedColumnUpdates::new("Parents", "Score", &parents),
            FixedColumnUpdates::new("Items", "Score", &children),
        ];
        assert_eq!(
            run(&source, &groups).err(),
            Some(EditError::Inspect(InspectError::Constraint(
                ConstraintViolation::MissingParent
            )))
        );

        let other = [update(0, 20, 21)];
        let candidate = run(
            &source,
            &[
                FixedColumnUpdates::new("Parents", "Score", &parents),
                FixedColumnUpdates::new("Items", "Score", &children),
                FixedColumnUpdates::new("Other", "Score", &other),
            ],
        )
        .unwrap();
        let mut expected = source.rows.clone();
        expected[1][0][1] = Value::Int32(11);
        expected[1][1][1] = Value::Int32(21);
        expected[0][0][1] = Value::Int32(11);
        expected[0][1][1] = Value::Int32(11);
        expected[2][0][1] = Value::Int32(21);
        assert_eq!(candidate.changed_rows(), 5);
        assert_eq!(candidate.before_bytes(), source.bytes);
        verify_rows(candidate.candidate_bytes(), &expected);
    }
}

#[test]
fn a_late_stale_value_remains_scoped_to_its_original_table() {
    let source = qualified::source(6);
    let parents = [update(0, 10, 11)];
    let stale = [update(0, 20, 11), update(1, 10, 11)];
    let groups = [
        FixedColumnUpdates::new("Parents", "Score", &parents),
        FixedColumnUpdates::new("Items", "Score", &stale),
    ];
    assert_eq!(run(&source, &groups).err(), Some(EditError::Conflict));
    verify_rows(&source.bytes, &source.rows);
}
