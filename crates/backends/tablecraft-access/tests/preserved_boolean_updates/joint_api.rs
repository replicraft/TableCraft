//! One original Access snapshot and one shared batch finish over frozen joint cases.
use super::{SCOPE, budget, joint_fixed_model as model};
use model::Recipe;
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
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError};
use tablecraft_schema::FieldConstraintViolation;
use tablecraft_sql::{ParseLimits, parse_query};

const MODES: [ScalarUpdateMode; 2] = [
    ScalarUpdateMode::Default,
    ScalarUpdateMode::Fixed(UpdateMode::Fitted),
];
const SQL_CASES: [(Recipe, &[&str], usize); 6] = [
    (
        Recipe::BooleanAndLarge,
        &[
            "[Large_Number]=CLngLng(-101)",
            "[Yes_Field]=([Large_Number]=CLngLng(1000))",
        ],
        1,
    ),
    (
        Recipe::SharedLowMask,
        &["[Date_Field]=NULL", "[Time_Field]=NULL"],
        1,
    ),
    (
        Recipe::SharedHighMask,
        &[
            "[Currency_Field]=NULL",
            "[Yes_Field]=TRUE",
            "[No_Field]=TRUE",
        ],
        1,
    ),
    (
        Recipe::Int32Boolean,
        &["[Large_Number]=101", "[Yes_Field]=([Large_Number]=1000)"],
        1,
    ),
    (
        Recipe::AllNullStay,
        &[
            "[Large_Number]=NULL",
            "[Currency_Field]=NULL",
            "[Date_Field]=NULL",
            "[Time_Field]=NULL",
            "[Yes_Field]=FALSE",
        ],
        1,
    ),
    (
        Recipe::EntirelyEmpty,
        &[
            "[Large_Number]=CLngLng(-101)",
            "[Date_Field]=NULL",
            "[Currency_Field]=NULL",
            "[Yes_Field]=TRUE",
            "[No_Field]=TRUE",
        ],
        0,
    ),
];

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

fn editor(
    case: &model::Case,
    mode: ScalarUpdateMode,
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AccessEngineeringEditor, NativeEditError> {
    let image = NativeImage::capture(&mut Cursor::new(&case.before), SCOPE, allowance, cancelled)?;
    AccessEngineeringEditor::new(image, "Items", mode, allowance, cancelled)
}

fn typed(
    case: &model::Case,
    mode: ScalarUpdateMode,
    reverse: bool,
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<EngineeringCopy, NativeEditError> {
    let editor = editor(case, mode, allowance, cancelled)?;
    let snapshot = editor.snapshot();
    assert_eq!(snapshot.rows().len(), 3);
    assert!(
        !snapshot
            .columns()
            .iter()
            .any(|column| { matches!(column.name(), "Calculated" | "Attachment_Field") })
    );
    let mut specifications = case.groups.clone();
    if reverse {
        specifications.reverse();
        for group in &mut specifications {
            group.updates.reverse();
        }
    }
    let mut updates = Vec::new();
    for group in &specifications {
        assert_eq!(group.table, snapshot.name());
        let position = snapshot
            .columns()
            .iter()
            .position(|column| column.name().eq_ignore_ascii_case(group.name))
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
        updates.push(requests);
    }
    let columns = specifications
        .iter()
        .zip(&updates)
        .map(|(group, updates)| ColumnUpdates {
            column: group.name,
            updates,
        })
        .collect::<Vec<_>>();
    editor.plan_columns(&columns, allowance, cancelled)
}

#[test]
fn preserved_joint_typed_batches_match_nineteen_images_in_both_modes_and_orders() {
    let mut images = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let Some(expected) = &case.expected else {
            continue;
        };
        for mode in MODES {
            for reverse in [false, true] {
                let copy = typed(&case, mode, reverse, &mut budget(), &|| false).unwrap();
                assert_image(copy.before_bytes(), &case.before);
                assert_image(copy.candidate_bytes(), expected);
                assert_eq!(copy.affected_rows(), case.changed);
                images += 1;
            }
        }
    }
    assert_eq!(images, 76);
}

#[test]
fn preserved_joint_sql_rhs_uses_original_rows_and_one_joint_finish() {
    let mut images = 0;
    for (recipe, assignments, matched) in SQL_CASES {
        let case = model::source(6, recipe);
        for reverse in [false, true] {
            let mut assignments = assignments.to_vec();
            if reverse {
                assignments.reverse();
            }
            let identifier = if matched == 0 { -1 } else { 1 };
            let statement = format!(
                "UPDATE [Items] SET {} WHERE [ID]={identifier}",
                assignments.join(","),
            );
            let query = parse_query(&statement, ParseLimits::default()).unwrap();
            for mode in MODES {
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
                assert_eq!(plan.matched_rows(), matched);
                let copy = plan.into_backend();
                assert_eq!(copy.affected_rows(), case.changed);
                assert_image(copy.before_bytes(), &case.before);
                assert_image(copy.candidate_bytes(), case.expected.as_ref().unwrap());
                images += 1;
            }
        }
    }
    assert_eq!(images, 24);
}

#[test]
fn preserved_joint_keeps_modes_header_five_and_snapshot_handles_closed() {
    let case = model::source(6, Recipe::BooleanAndLarge);
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
            typed(&case, mode, false, &mut budget(), &|| false),
            Err(NativeEditError::Unsupported)
        ));
    }
    let mut refused = 0;
    for recipe in model::RECIPES {
        let case = model::source(5, recipe);
        if case.expected.is_none() {
            continue;
        }
        assert!(matches!(
            typed(
                &case,
                ScalarUpdateMode::Default,
                false,
                &mut budget(),
                &|| false
            ),
            Err(NativeEditError::Edit(EditError::Unsupported(
                WriteBlocker::Column
            )))
        ));
        refused += 1;
    }
    assert_eq!(refused, 19);
    let owner = editor(&case, ScalarUpdateMode::Default, &mut budget(), &|| false).unwrap();
    let foreign_handle = owner.snapshot().row_handle(0).unwrap();
    let current = editor(&case, ScalarUpdateMode::Default, &mut budget(), &|| false).unwrap();
    let requests = [CellUpdate::new(foreign_handle, Value::Int64(-101))];
    assert!(matches!(
        current.plan_columns(
            &[ColumnUpdates {
                column: "Large_Number",
                updates: &requests
            }],
            &mut budget(),
            &|| false,
        ),
        Err(NativeEditError::Snapshot(_))
    ));
}

#[test]
fn preserved_joint_late_required_returns_no_candidate_in_either_order() {
    let case = model::source(6, Recipe::LateRequired);
    assert!(case.expected.is_none());
    for mode in MODES {
        for reverse in [false, true] {
            assert!(matches!(
                typed(&case, mode, reverse, &mut budget(), &|| false),
                Err(NativeEditError::Edit(EditError::Inspect(
                    InspectError::Constraint(ConstraintViolation::Field(
                        FieldConstraintViolation::Required
                    ))
                )))
            ));
        }
    }
}

#[test]
fn preserved_joint_cumulative_budget_limits_and_final_cancellation_are_typed() {
    let case = model::source(6, Recipe::OverlapRows);
    let polls = Cell::new(0);
    let mut measured = budget();
    typed(
        &case,
        ScalarUpdateMode::Default,
        false,
        &mut measured,
        &|| {
            polls.set(polls.get() + 1);
            false
        },
    )
    .unwrap();
    let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let used =
        resources.map(|resource| budget().remaining(resource) - measured.remaining(resource));
    assert!(used.iter().all(|amount| *amount > 0));
    let copy = typed(
        &case,
        ScalarUpdateMode::Default,
        false,
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
                false,
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
    let final_poll = polls.get();
    assert!(final_poll > 0);
    polls.set(0);
    assert!(matches!(
        typed(
            &case,
            ScalarUpdateMode::Default,
            false,
            &mut budget(),
            &|| {
                polls.set(polls.get() + 1);
                polls.get() == final_poll
            }
        ),
        Err(NativeEditError::Inspect(InspectError::Cancelled))
            | Err(NativeEditError::Snapshot(SnapshotError::Cancelled))
            | Err(NativeEditError::Edit(EditError::Inspect(
                InspectError::Cancelled
            )))
    ));
    assert_eq!(polls.get(), final_poll);
}
