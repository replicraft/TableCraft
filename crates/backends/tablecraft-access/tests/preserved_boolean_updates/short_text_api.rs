//! Focused consumers of the one actual independently authored Short Text owner.
use super::{SCOPE, budget, short_text_model as model};
use std::{cell::Cell, io::Cursor};
use tablecraft_access::{
    NativeEditError,
    engineering::{
        AccessEngineeringEditor, EngineeringCopy, EngineeringRequest, ScalarUpdateMode, UpdateMode,
        preview, sql,
    },
};
use tablecraft_ace::{EditError, FittedColumnUpdates, InspectError, NativeImage, SlotScalarUpdate};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{CellUpdate, EditableTable, SnapshotError};
use tablecraft_sql::{ParseLimits, parse_query};

fn value(text: Option<&str>) -> Value {
    match text {
        Some(text) => Value::Text(text.into()),
        None => Value::Null,
    }
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    let differences = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (actual, expected))| actual != expected)
        .take(8)
        .map(|(at, (actual, expected))| (at, *actual, *expected))
        .collect::<Vec<_>>();
    assert!(
        differences.is_empty(),
        "First image differences: {differences:?}"
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
        .position(|column| column.name() == "Short_Text")
        .unwrap();
    let id = snapshot
        .columns()
        .iter()
        .position(|column| column.name() == "ID")
        .unwrap();
    let row = case.framing.row;
    assert_eq!(snapshot.rows()[row][target], value(case.original));
    assert_eq!(
        matches!(snapshot.rows()[row][target], Value::Null),
        !case.framing.present
    );
    assert_eq!(snapshot.rows()[row][id], Value::Int32((row + 1) as i32));
    assert!(
        !snapshot
            .columns()
            .iter()
            .any(|column| column.name() == "Calculated")
    );
    let handle = snapshot.row_handle(row).unwrap();
    editor.plan_updates(
        "Short_Text",
        &[CellUpdate::new(handle, value(case.replacement))],
        allowance,
        cancelled,
    )
}

#[test]
fn projected_short_text_typed_uses_original_handles_and_preserves_exact_images() {
    for recipe in [model::Recipe::NullToValue, model::Recipe::Noop] {
        let case = model::source(6, recipe);
        let copy = typed(&case, ScalarUpdateMode::Default, &mut budget(), &|| false).unwrap();
        assert_image(copy.before_bytes(), &case.before);
        assert_image(copy.candidate_bytes(), case.expected.as_ref().unwrap());
        let payload = case.payload.unwrap();
        let start = case.framing.absolute + case.framing.offsets[case.framing.variable];
        assert_eq!(
            &copy.candidate_bytes()[start..start + payload.len()],
            payload
        );
        assert_eq!(
            copy.affected_rows(),
            usize::from(case.original != case.replacement)
        );
    }
}

#[test]
fn projected_short_text_shared_sql_uses_one_default_native_finish() {
    for (recipe, literal) in [
        (model::Recipe::ShorterUnicode, "'雪'"),
        (model::Recipe::ValueToNull, "NULL"),
    ] {
        let case = model::source(6, recipe);
        assert_eq!(case.framing.row, 0);
        let statement = format!("UPDATE Items SET [Short_Text]={literal} WHERE [ID]=1");
        let query = parse_query(&statement, ParseLimits::default()).unwrap();
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
            ScalarUpdateMode::Default,
            &mut allowance,
            None,
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.matched_rows(), 1);
        let copy = plan.into_backend();
        assert_image(copy.candidate_bytes(), case.expected.as_ref().unwrap());
    }

    let case = model::source(6, model::Recipe::ShorterNarrow);
    let mut allowance = budget();
    let image = NativeImage::capture(
        &mut Cursor::new(&case.before),
        SCOPE,
        &mut allowance,
        &|| false,
    )
    .unwrap();
    let query = parse_query(
        "UPDATE Items SET [Short_Text]='beta' WHERE [ID]=-1",
        ParseLimits::default(),
    )
    .unwrap();
    let empty = sql::preview_batch_update(
        image,
        query.query(),
        ScalarUpdateMode::Default,
        &mut allowance,
        None,
        &|| false,
    )
    .unwrap();
    assert_eq!(empty.matched_rows(), 0);
    let copy = empty.into_backend();
    assert_eq!(copy.affected_rows(), 0);
    assert_image(copy.candidate_bytes(), &case.before);
}

#[test]
fn projected_short_text_keeps_explicit_profiles_and_foreign_handles_closed() {
    let case = model::source(6, model::Recipe::ShorterNarrow);
    for mode in [
        ScalarUpdateMode::Fixed(UpdateMode::Fitted),
        ScalarUpdateMode::Fixed(UpdateMode::IndexGrowth),
        ScalarUpdateMode::Fixed(UpdateMode::Cascading),
        ScalarUpdateMode::Variable { index_growth: true },
        ScalarUpdateMode::LongValue,
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
    let foreign = first.snapshot().row_handle(case.framing.row).unwrap();
    assert!(matches!(
        second.plan_updates(
            "Short_Text",
            &[CellUpdate::new(foreign, Value::Text("beta".into()))],
            &mut budget(),
            &|| false
        ),
        Err(NativeEditError::Snapshot(_))
    ));

    for statement in [
        "UPDATE Items SET [Long_Text]='beta' WHERE [ID]=1",
        "UPDATE Items SET [Long_Text]='beta' WHERE [ID]=-1",
        "UPDATE Items SET [OLE_Object_Field]=NULL WHERE [ID]=1",
        "UPDATE Items SET [Calculated]='beta' WHERE [ID]=1",
        "UPDATE Items SET [Short_Text]=11 WHERE [ID]=1",
        "UPDATE Items SET [Short_Text]='beta',[Yes_Field]=True WHERE [ID]=1",
    ] {
        let query = parse_query(statement, ParseLimits::default()).unwrap();
        let mut allowance = budget();
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            SCOPE,
            &mut allowance,
            &|| false,
        )
        .unwrap();
        assert!(
            sql::preview_batch_update(
                image,
                query.query(),
                ScalarUpdateMode::Default,
                &mut allowance,
                None,
                &|| false,
            )
            .is_err()
        );
    }
}

#[test]
fn projected_short_text_cumulative_pools_and_final_cancel_preserve_typed_errors() {
    let case = model::source(6, model::Recipe::NullToValue);
    let mut measured = budget();
    let polls = Cell::new(0);
    typed(&case, ScalarUpdateMode::Default, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let used =
        resources.map(|resource| budget().remaining(resource) - measured.remaining(resource));
    assert!(used.iter().all(|amount| *amount > 0));
    let exact = typed(
        &case,
        ScalarUpdateMode::Default,
        &mut ResourceBudget::new(used[0], used[1], used[2]),
        &|| false,
    )
    .unwrap();
    assert_image(exact.candidate_bytes(), case.expected.as_ref().unwrap());
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
    let last = polls.get();
    assert!(last > 0);
    polls.set(0);
    assert!(matches!(
        typed(&case, ScalarUpdateMode::Default, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == last
        }),
        Err(NativeEditError::Inspect(InspectError::Cancelled))
            | Err(NativeEditError::Snapshot(SnapshotError::Cancelled))
            | Err(NativeEditError::Edit(EditError::Inspect(
                InspectError::Cancelled
            )))
    ));
    assert_eq!(polls.get(), last);
}

#[test]
fn shared_fitted_dispatch_preserves_exact_expected_values_for_projected_text() {
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let mut allowance = budget();
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            SCOPE,
            &mut allowance,
            &|| false,
        )
        .unwrap();
        let rows = [SlotScalarUpdate::new(
            case.framing.row,
            value(case.original),
            value(case.replacement),
        )];
        let groups = [FittedColumnUpdates::Variable {
            column: "Short_Text",
            updates: &rows,
        }];
        let result = preview(
            image,
            EngineeringRequest::FittedUpdate {
                table: "Items",
                groups: &groups,
            },
            &mut allowance,
            &|| false,
        );
        if let Some(expected) = &case.expected {
            assert_image(result.unwrap().candidate_bytes(), expected);
        } else {
            assert!(result.is_err());
        }
    }

    let case = model::source(6, model::Recipe::NullToValue);
    let mut allowance = budget();
    let image = NativeImage::capture(
        &mut Cursor::new(&case.before),
        SCOPE,
        &mut allowance,
        &|| false,
    )
    .unwrap();
    let rows = [SlotScalarUpdate::new(
        case.framing.row,
        value(Some("stale")),
        value(case.replacement),
    )];
    let groups = [FittedColumnUpdates::Variable {
        column: "Short_Text",
        updates: &rows,
    }];
    assert!(
        preview(
            image,
            EngineeringRequest::FittedUpdate {
                table: "Items",
                groups: &groups
            },
            &mut allowance,
            &|| false,
        )
        .is_err()
    );
}
