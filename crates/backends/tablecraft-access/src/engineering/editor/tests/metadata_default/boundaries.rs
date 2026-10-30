//! Routing boundaries reuse the existing genuine ordinary Memo/OLE source owner.
use super::super::{fixture, support};
use super::{AccessEngineeringEditor, ScalarUpdateMode, budget, model};
use crate::{NativeEditError, engineering::EngineeringCopy};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, WriteBlocker, inspect_table_schema, read_table,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable};
use tablecraft_model::CatalogScope;

#[path = "../../../../../../../engines/tablecraft-ace/tests/ace16_long_value_updates_contract/model/source.rs"]
mod long_value_source;

fn capture(
    bytes: &[u8],
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeImage, NativeEditError> {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        work,
        cancelled,
    )
    .map_err(Into::into)
}

#[test]
fn qualify_original_memo_ole_source_before_default_routing_refusal() {
    for code in [5, 6] {
        let before = long_value_source::source(code, 1);
        let schema = inspect_table_schema(
            &mut Cursor::new(&before),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(schema.columns()[1].name(), "Memo");
        assert_eq!(schema.columns()[1].native_type(), 12);
        assert_eq!(schema.columns()[2].name(), "Ole");
        assert_eq!(schema.columns()[2].native_type(), 11);
        let actual = read_table(
            &mut Cursor::new(&before),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            actual.rows(),
            &[vec![
                Value::Int32(1),
                Value::Text("A".to_owned()),
                Value::Binary(vec![7])
            ]]
        );
        let checked = tablecraft_ace::check_table(
            &mut Cursor::new(&before),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), 1);
    }
}

#[test]
fn default_memo_ole_storage_refuses_zero_and_nonzero_single_and_joint_requests() {
    for code in [5, 6] {
        let before = long_value_source::source(code, 1);
        for (name, replacement) in [
            ("Memo", Value::Text("A".to_owned())),
            ("Ole", Value::Binary(vec![7])),
        ] {
            for selected in [false, true] {
                // Positive explicit LVAL admission proves the original is not
                // rejected for an unrelated incomplete source inventory.
                let explicit = AccessEngineeringEditor::new(
                    capture(&before, &mut budget(), &|| false).unwrap(),
                    "Items",
                    ScalarUpdateMode::LongValue,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let rows = if selected {
                    vec![CellUpdate::new(
                        explicit.snapshot().row_handle(0).unwrap(),
                        replacement.clone(),
                    )]
                } else {
                    Vec::new()
                };
                assert!(
                    explicit
                        .plan_updates(name, &rows, &mut budget(), &|| false)
                        .is_ok()
                );

                for joint in [false, true] {
                    let editor = AccessEngineeringEditor::new(
                        capture(&before, &mut budget(), &|| false).unwrap(),
                        "Items",
                        ScalarUpdateMode::Default,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let row = editor.snapshot().row_handle(0).unwrap();
                    let rows = if selected {
                        vec![CellUpdate::new(row, replacement.clone())]
                    } else {
                        Vec::new()
                    };
                    let fixed = if selected {
                        vec![CellUpdate::new(row, Value::Int32(1))]
                    } else {
                        Vec::new()
                    };
                    let mut groups = vec![ColumnUpdates {
                        column: name,
                        updates: &rows,
                    }];
                    if joint {
                        groups.push(ColumnUpdates {
                            column: "Id",
                            updates: &fixed,
                        });
                    }
                    assert!(matches!(
                        editor.plan_columns(&groups, &mut budget(), &|| false),
                        Err(NativeEditError::Edit(EditError::Unsupported(
                            WriteBlocker::Column
                        )))
                    ));
                }
            }
        }
    }
}

fn run_default(
    case: &model::Case,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<EngineeringCopy, NativeEditError> {
    let image = capture(&case.before, work, cancelled)?;
    let editor =
        AccessEngineeringEditor::new(image, "Items", ScalarUpdateMode::Default, work, cancelled)?;
    let mut age = Vec::new();
    let mut name = Vec::new();
    for ordinal in &case.changed {
        let row = editor.snapshot().row_handle(*ordinal).unwrap();
        age.push(CellUpdate::new(
            row,
            case.final_rows[*ordinal]
                .age
                .map_or(Value::Null, Value::Int32),
        ));
        name.push(CellUpdate::new(
            row,
            model::name_value(&case.final_rows[*ordinal], case.family),
        ));
    }
    editor.plan_columns(
        &[
            ColumnUpdates {
                column: "Age",
                updates: &age,
            },
            ColumnUpdates {
                column: "Name",
                updates: &name,
            },
        ],
        work,
        cancelled,
    )
}

#[test]
fn default_route_cumulative_pools_and_final_cancellation_cover_capture_through_finish() {
    let case = model::case(
        6,
        model::Family::Modern,
        true,
        model::Recipe::Permutation,
        true,
    );
    let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let limits = [256 * 1024 * 1024, 100_000, 100_000_000];
    let mut work = ResourceBudget::new(limits[0], limits[1], limits[2]);
    let polls = Cell::new(0);
    let copy = run_default(&case, &mut work, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(copy.candidate_bytes(), case.expected);
    let needed =
        std::array::from_fn::<_, 3, _>(|index| limits[index] - work.remaining(resources[index]));

    for index in 0..3 {
        assert!(needed[index] > 0);
        let mut exact = limits;
        exact[index] = needed[index];
        let copy = run_default(
            &case,
            &mut ResourceBudget::new(exact[0], exact[1], exact[2]),
            &|| false,
        )
        .unwrap();
        assert_eq!(copy.candidate_bytes(), case.expected);
        exact[index] -= 1;
        assert!(
            run_default(
                &case,
                &mut ResourceBudget::new(exact[0], exact[1], exact[2]),
                &|| false
            )
            .is_err()
        );
    }

    let final_poll = polls.get();
    let polls = Cell::new(0);
    assert!(matches!(
        run_default(&case, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == final_poll
        }),
        Err(NativeEditError::Edit(EditError::Inspect(
            InspectError::Cancelled
        )))
    ));
}
