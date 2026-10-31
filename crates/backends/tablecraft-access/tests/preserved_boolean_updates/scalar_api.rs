//! Shared Access services over the actual frozen ordinary eight-byte model.
use super::{SCOPE, budget, fixed_scalar_model as model};
use std::{cell::Cell, io::Cursor};
use tablecraft_access::{
    NativeEditError,
    engineering::{AccessEngineeringEditor, EngineeringCopy, ScalarUpdateMode, UpdateMode, sql},
};
use tablecraft_ace::{ConstraintViolation, EditError, InspectError, NativeImage, WriteBlocker};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{CellUpdate, EditableTable, SnapshotError};
use tablecraft_schema::FieldConstraintViolation;
use tablecraft_sql::{ParseLimits, parse_query};

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    let differences = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (left, right))| left != right)
        .take(8)
        .map(|(at, (left, right))| (at, *left, *right))
        .collect::<Vec<_>>();
    assert!(
        differences.is_empty(),
        "first image differences {differences:?}"
    );
}

fn typed(
    case: &model::Case,
    mode: ScalarUpdateMode,
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<EngineeringCopy, NativeEditError> {
    let image = NativeImage::capture(&mut Cursor::new(&case.before), SCOPE, allowance, cancelled)?;
    let editor = AccessEngineeringEditor::new(image, "Items", mode, allowance, cancelled)?;
    let snapshot = editor.snapshot();
    let target = snapshot
        .columns()
        .iter()
        .position(|field| field.name() == case.target.name())
        .unwrap();
    assert!(
        !snapshot
            .columns()
            .iter()
            .any(|field| field.name() == "Calculated")
    );
    assert!(
        !snapshot
            .columns()
            .iter()
            .any(|field| field.name() == "Attachment_Field")
    );
    let updates = (0..case.requested_rows)
        .map(|row| {
            assert_eq!(
                snapshot.rows()[row][target],
                case.target.value(case.initial[row])
            );
            CellUpdate::new(
                snapshot.row_handle(row).unwrap(),
                case.target.value(case.replacement[row]),
            )
        })
        .collect::<Vec<_>>();
    editor.plan_updates(case.target.name(), &updates, allowance, cancelled)
}

#[test]
fn preserved_scalar_typed_default_and_fitted_match_seventy_literal_images() {
    let mut copies = 0;
    let mut required = 0;
    for (target, recipe) in model::cases() {
        let case = model::source(6, target, recipe);
        for mode in [
            ScalarUpdateMode::Default,
            ScalarUpdateMode::Fixed(UpdateMode::Fitted),
        ] {
            let result = typed(&case, mode, &mut budget(), &|| false);
            if let Some(expected) = &case.expected {
                let copy = result.unwrap();
                assert_image(copy.before_bytes(), &case.before);
                assert_image(copy.candidate_bytes(), expected);
                assert_eq!(copy.affected_rows(), case.changed);
                copies += 1;
            } else {
                assert_eq!(recipe, model::Recipe::RequiredRefusal);
                assert!(matches!(
                    result,
                    Err(NativeEditError::Edit(EditError::Inspect(
                        InspectError::Constraint(ConstraintViolation::Field(
                            FieldConstraintViolation::Required
                        ))
                    )))
                ));
                required += 1;
            }
        }
    }
    assert_eq!((copies, required), (70, 8));
}

#[test]
fn preserved_scalar_shared_sql_uses_existing_explicit_conversions_in_both_modes() {
    let mut copies = 0;
    for (target, recipe, expression) in [
        (
            model::Target::LargeNumber,
            model::Recipe::Change,
            "CLngLng(-101)",
        ),
        (
            model::Target::Date,
            model::Recipe::Change,
            "#1899-12-31 06:00:00#",
        ),
        (model::Target::Time, model::Recipe::ValueToNull, "NULL"),
        (
            model::Target::Currency,
            model::Recipe::Change,
            "CCur(-1.0001)",
        ),
    ] {
        let case = model::source(6, target, recipe);
        let statement = format!(
            "UPDATE Items SET [{}]={expression} WHERE [ID]=1",
            target.name()
        );
        let query = parse_query(&statement, ParseLimits::default()).unwrap();
        for mode in [
            ScalarUpdateMode::Default,
            ScalarUpdateMode::Fixed(UpdateMode::Fitted),
        ] {
            let mut allowance = budget();
            let image = NativeImage::capture(
                &mut Cursor::new(&case.before),
                SCOPE,
                &mut allowance,
                &|| false,
            )
            .unwrap();
            let plan = sql::preview_batch_update(
                image,
                query.query(),
                mode,
                &mut allowance,
                None,
                &|| false,
            )
            .unwrap();
            assert_eq!(plan.matched_rows(), 1);
            let copy = plan.into_backend();
            assert_eq!(copy.affected_rows(), 1);
            assert_image(copy.candidate_bytes(), case.expected.as_ref().unwrap());
            copies += 1;
        }
    }
    assert_eq!(copies, 8);
}

#[test]
fn preserved_scalar_service_modes_handles_and_joint_complete_images() {
    let case = model::source(6, model::Target::Currency, model::Recipe::Change);
    for mode in [
        ScalarUpdateMode::Variable {
            index_growth: false,
        },
        ScalarUpdateMode::Variable { index_growth: true },
        ScalarUpdateMode::LongValue,
        ScalarUpdateMode::Fixed(UpdateMode::IndexGrowth),
        ScalarUpdateMode::Fixed(UpdateMode::Cascading),
    ] {
        assert!(matches!(
            typed(&case, mode, &mut budget(), &|| false),
            Err(NativeEditError::Unsupported)
        ));
    }
    let create = || {
        let mut allowance = budget();
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            SCOPE,
            &mut allowance,
            &|| false,
        )
        .unwrap();
        AccessEngineeringEditor::new(
            image,
            "Items",
            ScalarUpdateMode::Default,
            &mut allowance,
            &|| false,
        )
        .unwrap()
    };
    let first = create();
    let second = create();
    let foreign = first.snapshot().row_handle(0).unwrap();
    assert!(matches!(
        second.plan_updates(
            "Currency_Field",
            &[CellUpdate::new(
                foreign,
                case.target.value(case.replacement[0])
            )],
            &mut budget(),
            &|| false
        ),
        Err(NativeEditError::Snapshot(_))
    ));
    let editor = create();
    let handle = editor.snapshot().row_handle(0).unwrap();
    assert!(matches!(
        editor.plan_updates(
            "Currency_Field",
            &[CellUpdate::new(handle, Value::Int64(-10001))],
            &mut budget(),
            &|| false
        ),
        Err(NativeEditError::Unsupported)
    ));

    let date = model::source(6, model::Target::Date, model::Recipe::Change);
    assert_image(&date.before, &case.before);
    let date_expected = date.expected.as_ref().unwrap();
    let mut joint_expected = case.expected.as_ref().unwrap().clone();
    // Compose two independently frozen single-field images with identical BEFORE.
    // Their actual target spans are disjoint; no producer output is an input.
    for offset in 0..case.before.len() {
        if date_expected[offset] != case.before[offset] {
            assert_eq!(joint_expected[offset], case.before[offset]);
            joint_expected[offset] = date_expected[offset];
        }
    }
    for (statement, matched) in [
        (
            "UPDATE Items SET [Currency_Field]=CCur(-1.0001),[Date_Field]=#1899-12-31 06:00:00# WHERE [ID]=1",
            1,
        ),
        (
            "UPDATE Items SET [Currency_Field]=CCur(-1.0001),[Date_Field]=#1899-12-31 06:00:00# WHERE [ID]=-1",
            0,
        ),
    ] {
        let mut allowance = budget();
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            SCOPE,
            &mut allowance,
            &|| false,
        )
        .unwrap();
        let query = parse_query(statement, ParseLimits::default()).unwrap();
        let plan = sql::preview_batch_update(
            image,
            query.query(),
            ScalarUpdateMode::Default,
            &mut allowance,
            None,
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.matched_rows(), matched);
        let copy = plan.into_backend();
        assert_eq!(copy.affected_rows(), matched);
        assert_image(
            copy.candidate_bytes(),
            if matched == 0 {
                &case.before
            } else {
                &joint_expected
            },
        );
    }
    let other = model::source(5, model::Target::Currency, model::Recipe::Change);
    assert!(matches!(
        typed(&other, ScalarUpdateMode::Default, &mut budget(), &|| false),
        Err(NativeEditError::Edit(EditError::Unsupported(
            WriteBlocker::Column
        )))
    ));
}

#[test]
fn preserved_scalar_service_cumulative_pools_and_final_cancellation_are_typed() {
    let case = model::source(6, model::Target::Date, model::Recipe::NullToSameBytes);
    let polls = Cell::new(0);
    let mut measured = budget();
    typed(&case, ScalarUpdateMode::Default, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let used =
        resources.map(|resource| budget().remaining(resource) - measured.remaining(resource));
    assert!(used.iter().all(|amount| *amount > 0));
    let copy = typed(
        &case,
        ScalarUpdateMode::Default,
        &mut ResourceBudget::new(used[0], used[1], used[2]),
        &|| false,
    )
    .unwrap();
    assert_image(copy.candidate_bytes(), case.expected.as_ref().unwrap());
    for position in 0..resources.len() {
        let mut limits = used;
        limits[position] -= 1;
        assert!(matches!(
            typed(
                &case,
                ScalarUpdateMode::Default,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false
            ),
            Err(NativeEditError::Inspect(InspectError::Limit(_)))
                | Err(NativeEditError::Snapshot(SnapshotError::Limit(_)))
                | Err(NativeEditError::Edit(EditError::Inspect(
                    InspectError::Limit(_)
                )))
        ));
    }
    let final_poll = polls.get();
    assert!(final_poll > 0);
    polls.set(0);
    assert!(matches!(
        typed(&case, ScalarUpdateMode::Default, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == final_poll
        }),
        Err(NativeEditError::Inspect(InspectError::Cancelled))
            | Err(NativeEditError::Snapshot(SnapshotError::Cancelled))
            | Err(NativeEditError::Edit(EditError::Inspect(
                InspectError::Cancelled
            )))
    ));
    assert_eq!(polls.get(), final_poll);
}
