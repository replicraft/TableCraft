//! Snapshot-bound joint Text/fixed updates over independent original-image cases.
use super::{SCOPE, budget, joint_text_model as model};
use std::{cell::Cell, io::Cursor};
use tablecraft_access::{
    NativeEditError,
    engineering::{AccessEngineeringEditor, EngineeringCopy, ScalarUpdateMode, UpdateMode, sql},
};
use tablecraft_ace::{EditError, InspectError, NativeImage};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError};
use tablecraft_sql::{ParseLimits, parse_query};

fn text(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |value| Value::Text(value.to_owned()))
}

fn run(
    case: &model::Case,
    reverse: bool,
    mode: ScalarUpdateMode,
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<EngineeringCopy, NativeEditError> {
    let image = NativeImage::capture(&mut Cursor::new(&case.before), SCOPE, allowance, cancelled)?;
    let editor = AccessEngineeringEditor::new(image, "Items", mode, allowance, cancelled)?;
    let snapshot = editor.snapshot();
    let mut selected = Vec::new();
    let mut rows = Vec::new();
    let text_position = snapshot
        .columns()
        .iter()
        .position(|column| column.name() == "Short_Text")
        .unwrap();
    let mut requests = Vec::new();
    for assignment in &case.texts {
        assert_eq!(
            snapshot.rows()[assignment.row][text_position],
            text(assignment.expected)
        );
        requests.push(CellUpdate::new(
            snapshot.row_handle(assignment.row).unwrap(),
            text(assignment.replacement),
        ));
    }
    selected.push("Short_Text");
    rows.push(requests);
    for group in &case.fixed {
        let position = snapshot
            .columns()
            .iter()
            .position(|column| column.name() == group.name)
            .unwrap();
        let mut requests = Vec::new();
        for assignment in &group.updates {
            assert_eq!(
                snapshot.rows()[assignment.row][position],
                assignment.expected.value()
            );
            requests.push(CellUpdate::new(
                snapshot.row_handle(assignment.row).unwrap(),
                assignment.replacement.value(),
            ));
        }
        selected.push(group.name);
        rows.push(requests);
    }
    if reverse {
        selected.reverse();
        rows.reverse();
        for requests in &mut rows {
            requests.reverse();
        }
    }
    let groups = selected
        .iter()
        .zip(&rows)
        .map(|(column, updates)| ColumnUpdates { column, updates })
        .collect::<Vec<_>>();
    editor.plan_columns(&groups, allowance, cancelled)
}

#[test]
fn joint_text_default_editor_matches_all_complete_positive_images_in_both_orders() {
    let mut copies = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let Some(expected) = &case.expected else {
            continue;
        };
        for reverse in [false, true] {
            let copy = run(
                &case,
                reverse,
                ScalarUpdateMode::Default,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(copy.before_bytes() == case.before);
            assert!(copy.candidate_bytes() == expected);
            assert_eq!(copy.affected_rows(), case.changed);
            copies += 1;
        }
    }
    assert_eq!(copies, 36);
}

#[test]
fn joint_text_editor_retains_explicit_mode_refusals() {
    let case = model::RECIPES
        .into_iter()
        .map(|recipe| model::source(6, recipe))
        .find(|case| case.expected.is_some() && !case.texts.is_empty())
        .unwrap();
    for mode in [
        ScalarUpdateMode::Fixed(UpdateMode::Fitted),
        ScalarUpdateMode::Fixed(UpdateMode::IndexGrowth),
        ScalarUpdateMode::Fixed(UpdateMode::Cascading),
        ScalarUpdateMode::Variable {
            index_growth: false,
        },
        ScalarUpdateMode::Variable { index_growth: true },
        ScalarUpdateMode::LongValue,
    ] {
        assert!(run(&case, false, mode, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn joint_text_editor_keeps_cumulative_limits_and_final_cancellation() {
    let case = model::RECIPES
        .into_iter()
        .map(|recipe| model::source(6, recipe))
        .find(|case| case.expected.is_some() && case.changed > 0)
        .unwrap();
    let mut measured = budget();
    let polls = Cell::new(0);
    run(
        &case,
        false,
        ScalarUpdateMode::Default,
        &mut measured,
        &|| {
            polls.set(polls.get() + 1);
            false
        },
    )
    .unwrap();
    let used = [Resource::Bytes, Resource::Rows, Resource::Steps]
        .map(|resource| budget().remaining(resource) - measured.remaining(resource));
    assert!(used.iter().all(|amount| *amount > 0));
    let exact = run(
        &case,
        false,
        ScalarUpdateMode::Default,
        &mut ResourceBudget::new(used[0], used[1], used[2]),
        &|| false,
    )
    .unwrap();
    assert!(exact.candidate_bytes() == case.expected.as_ref().unwrap());
    for position in 0..used.len() {
        let mut limits = used;
        limits[position] -= 1;
        assert!(matches!(
            run(
                &case,
                false,
                ScalarUpdateMode::Default,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false,
            ),
            Err(NativeEditError::Inspect(InspectError::Limit(_)))
                | Err(NativeEditError::Snapshot(SnapshotError::Limit(_)))
                | Err(NativeEditError::Edit(EditError::Inspect(
                    InspectError::Limit(_)
                )))
        ));
    }
    let last = polls.get();
    polls.set(0);
    assert!(matches!(
        run(
            &case,
            false,
            ScalarUpdateMode::Default,
            &mut budget(),
            &|| {
                polls.set(polls.get() + 1);
                polls.get() == last
            }
        ),
        Err(NativeEditError::Inspect(InspectError::Cancelled))
            | Err(NativeEditError::Snapshot(SnapshotError::Cancelled))
            | Err(NativeEditError::Edit(EditError::Inspect(
                InspectError::Cancelled
            )))
    ));
    assert_eq!(polls.get(), last);
}

#[test]
fn joint_text_sql_uses_original_rhs_and_qualifies_true_empty_groups() {
    for (recipe, assignments, matched) in [
        (
            model::Recipe::BetaBoolean,
            vec!["[Short_Text]='beta'", "[Yes_Field]=(Len([Short_Text])=5)"],
            1,
        ),
        (
            model::Recipe::EntirelyEmpty,
            vec![
                "[Short_Text]='beta'",
                "[Large_Number]=CLngLng(-101)",
                "[Date_Field]=NULL",
                "[Currency_Field]=NULL",
                "[Yes_Field]=TRUE",
            ],
            0,
        ),
    ] {
        let case = model::source(6, recipe);
        for reverse in [false, true] {
            let mut assignments = assignments.clone();
            if reverse {
                assignments.reverse();
            }
            let identifier = if matched == 0 { -1 } else { 1 };
            let statement = format!(
                "UPDATE Items SET {} WHERE [ID]={identifier}",
                assignments.join(","),
            );
            let query = parse_query(&statement, ParseLimits::default()).unwrap();
            let mut allowance = budget();
            let image = NativeImage::capture(
                &mut Cursor::new(&case.before),
                SCOPE,
                &mut allowance,
                &|| false,
            )
            .unwrap();
            let result = sql::preview_batch_update(
                image,
                query.query(),
                ScalarUpdateMode::Default,
                &mut allowance,
                None,
                &|| false,
            )
            .unwrap();
            assert_eq!(result.matched_rows(), matched);
            let copy = result.into_backend();
            assert_eq!(copy.affected_rows(), case.changed);
            assert!(copy.before_bytes() == case.before);
            assert!(copy.candidate_bytes() == case.expected.as_ref().unwrap());
        }
    }
}
