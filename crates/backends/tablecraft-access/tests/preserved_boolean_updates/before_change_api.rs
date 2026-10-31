//! Typed/shared SQL acceptance over the single frozen BeforeChange matrix.
use super::{SCOPE, before_change_fixtures::model, budget};
use model::{Recipe, Refusal};
use std::{cell::Cell, io::Cursor};
use tablecraft_access::{
    NativeEditError,
    engineering::{
        AccessEngineeringEditor, ScalarUpdateMode, UpdateMode,
        guards::before_change::{self, BeforeChangeBackendError, BeforeChangeBatchEditor},
    },
};
use tablecraft_ace::{
    Ace16BeforeChangePreview, BeforeChangePreviewError, BeforeEventGuardError,
    BeforeEventGuardSelector, BeforeGuardTraceKind, ConstraintViolation, EditError, InspectError,
    NativeImage, WriteBlocker,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Value},
};
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError};
use tablecraft_schema::FieldConstraintViolation;
use tablecraft_sql::{ParseLimits, parse_query};

const SQL_CASES: [(Recipe, &[&str], i32, usize); 7] = [
    (
        Recipe::SequentialSeeded,
        &["[Large_Number]=10", "[Currency_Field]=CCur(-1.0001)"],
        1,
        1,
    ),
    (
        Recipe::SequentialNullSeeds,
        &[
            "[Large_Number]=NULL",
            "[Currency_Field]=NULL",
            "[Yes_Field]=([Large_Number]=1000)",
        ],
        1,
        1,
    ),
    (
        Recipe::SequentialRestoresOriginal,
        &["[Large_Number]=-1", "[Currency_Field]=[Currency_Field]"],
        1,
        1,
    ),
    (
        Recipe::DerivedOnlyWide,
        &["[Currency_Field]=CCur(-1.0001)"],
        1,
        1,
    ),
    (Recipe::CounterOperand, &["[Yes_Field]=TRUE"], 1, 1),
    (
        Recipe::EntirelyEmpty,
        &[
            "[Large_Number]=[Large_Number]",
            "[Time_Field]=[Time_Field]",
            "[Yes_Field]=[Yes_Field]",
        ],
        -1,
        0,
    ),
    (
        Recipe::RequiredRepair,
        &["[Large_Number]=NULL", "[Yes_Field]=TRUE"],
        1,
        1,
    ),
];

fn selectors(case: &model::Case) -> Vec<BeforeEventGuardSelector<'_>> {
    case.source
        .selection
        .iter()
        .map(|&position| BeforeEventGuardSelector::new(3, &case.source.properties[position].0))
        .collect()
}

fn image(
    case: &model::Case,
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeImage, BeforeChangeBackendError> {
    NativeImage::capture(
        &mut Cursor::new(&case.source.before),
        SCOPE,
        allowance,
        cancelled,
    )
    .map_err(NativeEditError::from)
    .map_err(BeforeChangeBackendError::Native)
}

fn typed(
    case: &model::Case,
    reverse: bool,
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16BeforeChangePreview, BeforeChangeBackendError> {
    let selected = selectors(case);
    let editor = BeforeChangeBatchEditor::new(
        image(case, allowance, cancelled)?,
        "Items",
        &selected,
        allowance,
        cancelled,
    )?;
    let snapshot = editor.snapshot();
    assert_eq!(snapshot.rows().len(), 3);
    assert!(
        !snapshot
            .columns()
            .iter()
            .any(|field| { matches!(field.name(), "Calculated" | "Attachment_Field") })
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
            .position(|field| field.name().eq_ignore_ascii_case(group.name))
            .unwrap();
        let mut rows = Vec::new();
        for assignment in &group.updates {
            assert_eq!(
                snapshot.rows()[assignment.row][position],
                assignment.expected.value()
            );
            rows.push(CellUpdate::new(
                snapshot.row_handle(assignment.row).unwrap(),
                assignment.replacement.value(),
            ));
        }
        updates.push(rows);
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
        "first whole-image differences {differences:?}"
    );
}

fn assert_report(copy: &Ace16BeforeChangePreview, case: &model::Case) {
    assert_image(copy.before_bytes(), &case.source.before);
    assert_image(copy.candidate_bytes(), case.expected.as_ref().unwrap());
    assert_eq!(copy.changed_rows(), case.changed);
    assert_eq!(copy.programs().len(), case.source.selection.len());
    let rows = copy
        .trace()
        .iter()
        .filter(|step| step.kind() == BeforeGuardTraceKind::Begin)
        .map(|step| step.row().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows, case.event_rows);
    let assignments = copy
        .assignments()
        .iter()
        .map(|entry| (entry.ordinal(), entry.column()))
        .collect::<Vec<_>>();
    assert_eq!(
        assignments,
        case.assignments
            .iter()
            .map(|entry| (entry.row, entry.column))
            .collect::<Vec<_>>()
    );
}

#[test]
fn before_change_rich_typed_batches_match_fifteen_whole_images_in_both_orders() {
    let mut images = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        if case.expected.is_none() {
            continue;
        }
        for reverse in [false, true] {
            let copy = typed(&case, reverse, &mut budget(), &|| false).unwrap();
            assert_report(&copy, &case);
            images += 1;
        }
    }
    assert_eq!(images, 30);
}

#[test]
fn before_change_rich_sql_uses_original_rhs_and_sequential_current_with_frozen_old() {
    let mut images = 0;
    for (recipe, assignments, identifier, matched) in SQL_CASES {
        let case = model::source(6, recipe);
        for reverse in [false, true] {
            let mut assignments = assignments.to_vec();
            if reverse {
                assignments.reverse();
            }
            let sql = format!(
                "UPDATE [Items] SET {} WHERE [ID]={identifier}",
                assignments.join(",")
            );
            let query = parse_query(&sql, ParseLimits::default()).unwrap();
            let selected = selectors(&case);
            let mut allowance = budget();
            let plan = before_change::preview_batch_update(
                image(&case, &mut allowance, &|| false).unwrap(),
                query.query(),
                &selected,
                &mut allowance,
                None,
                &|| false,
            )
            .unwrap();
            assert_eq!(plan.matched_rows(), matched);
            assert_report(&plan.into_backend(), &case);
            images += 1;
        }
    }
    assert_eq!(images, 14);
}

fn required(error: &BeforeChangeBackendError) -> bool {
    matches!(
        error,
        BeforeChangeBackendError::Preview(BeforeChangePreviewError::Edit(EditError::Inspect(
            InspectError::Constraint(ConstraintViolation::Field(
                FieldConstraintViolation::Required
            ))
        )))
    )
}

#[test]
fn before_change_rich_refusals_keep_required_source_content_and_dormant_targets_closed() {
    let mut refusals = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let Some(refusal) = case.refusal else {
            continue;
        };
        for reverse in [false, true] {
            let error = typed(&case, reverse, &mut budget(), &|| false)
                .err()
                .expect("no candidate");
            match refusal {
                Refusal::Required => assert!(required(&error), "{error:?}"),
                Refusal::Rejected { number, .. } => assert!(matches!(error,
                    BeforeChangeBackendError::Preview(BeforeChangePreviewError::Rejected(ref value))
                    if value.number() == number
                )),
                _ => assert!(
                    matches!(error, BeforeChangeBackendError::Preview(_)),
                    "{error:?}"
                ),
            }
            refusals += 1;
        }
    }
    assert_eq!(refusals, 28);

    // Zero matches still qualify the dormant ID target. Its AutoNumber metadata
    // refuses with Column before the separate index restriction is considered.
    let case = model::source(6, Recipe::DormantIndexedTarget);
    let query = parse_query(
        "UPDATE [Items] SET [Currency_Field]=[Currency_Field] WHERE [ID]=-1",
        ParseLimits::default(),
    )
    .unwrap();
    let selected = selectors(&case);
    let mut allowance = budget();
    let error = before_change::preview_batch_update(
        image(&case, &mut allowance, &|| false).unwrap(),
        query.query(),
        &selected,
        &mut allowance,
        None,
        &|| false,
    )
    .err()
    .expect("dormant target must refuse even with no rows");
    assert!(
        matches!(
            error,
            tablecraft_query::UpdateError::Backend(BeforeChangeBackendError::Preview(
                BeforeChangePreviewError::Preparation(BeforeEventGuardError::Intent(
                    EditError::Unsupported(WriteBlocker::Column)
                ))
            ))
        ),
        "actual dormant-target refusal: {error:?}"
    );

    let case = model::source(6, Recipe::RequiredUnrepaired);
    let query = parse_query(
        "UPDATE [Items] SET [Large_Number]=NULL,[Yes_Field]=TRUE WHERE [ID]=1",
        ParseLimits::default(),
    )
    .unwrap();
    let selected = selectors(&case);
    let mut allowance = budget();
    let error = before_change::preview_batch_update(
        image(&case, &mut allowance, &|| false).unwrap(),
        query.query(),
        &selected,
        &mut allowance,
        None,
        &|| false,
    )
    .err()
    .expect("final Required must refuse");
    let tablecraft_query::UpdateError::Backend(error) = error else {
        panic!("expected backend Required refusal");
    };
    assert!(required(&error), "{error:?}");
}

#[test]
fn before_change_rich_preserves_snapshot_handles_modes_and_header_five_refusal() {
    let case = model::source(6, Recipe::SequentialSeeded);
    let selected = selectors(&case);
    let mut allowance = budget();
    let first = BeforeChangeBatchEditor::new(
        image(&case, &mut allowance, &|| false).unwrap(),
        "Items",
        &selected,
        &mut allowance,
        &|| false,
    )
    .unwrap();
    let foreign = first.snapshot().row_handle(0).unwrap();
    let second = BeforeChangeBatchEditor::new(
        image(&case, &mut allowance, &|| false).unwrap(),
        "Items",
        &selected,
        &mut allowance,
        &|| false,
    )
    .unwrap();
    let rows = [CellUpdate::new(
        foreign,
        Value::Currency(Currency::from_scaled(-10001)),
    )];
    let error = second
        .plan_columns(
            &[ColumnUpdates {
                column: "Currency_Field",
                updates: &rows,
            }],
            &mut allowance,
            &|| false,
        )
        .err()
        .expect("foreign handle");
    assert!(matches!(
        error,
        BeforeChangeBackendError::Native(NativeEditError::Snapshot(_))
    ));

    // Existing implicit/explicit generic modes never execute the selected macro.
    for mode in [
        ScalarUpdateMode::Default,
        ScalarUpdateMode::Fixed(UpdateMode::Fitted),
        ScalarUpdateMode::Fixed(UpdateMode::IndexGrowth),
        ScalarUpdateMode::Variable {
            index_growth: false,
        },
        ScalarUpdateMode::Variable { index_growth: true },
        ScalarUpdateMode::LongValue,
    ] {
        let mut allowance = budget();
        let editor = AccessEngineeringEditor::new(
            image(&case, &mut allowance, &|| false).unwrap(),
            "Items",
            mode,
            &mut allowance,
            &|| false,
        )
        .unwrap();
        let rows = [CellUpdate::new(
            editor.snapshot().row_handle(0).unwrap(),
            Value::Currency(Currency::from_scaled(-10001)),
        )];
        let error = editor
            .plan_columns(
                &[ColumnUpdates {
                    column: "Currency_Field",
                    updates: &rows,
                }],
                &mut allowance,
                &|| false,
            )
            .err()
            .expect("no implicit macro execution");
        assert!(matches!(
            error,
            NativeEditError::Unsupported
                | NativeEditError::Edit(EditError::Unsupported(WriteBlocker::ActiveContent))
        ));
    }
    let mut refusals = 0;
    for recipe in model::RECIPES {
        let positive = model::source(6, recipe);
        if positive.expected.is_none() {
            continue;
        }
        let case = model::source(5, recipe);
        assert!(typed(&case, false, &mut budget(), &|| false).is_err());
        refusals += 1;
    }
    assert_eq!(refusals, 15);
}

fn limited(error: &BeforeChangeBackendError) -> bool {
    matches!(
        error,
        BeforeChangeBackendError::Native(
            NativeEditError::Inspect(InspectError::Limit(_))
                | NativeEditError::Snapshot(SnapshotError::Limit(_))
                | NativeEditError::Edit(EditError::Inspect(InspectError::Limit(_)))
        ) | BeforeChangeBackendError::Preview(
            BeforeChangePreviewError::Preparation(BeforeEventGuardError::Inspect(
                InspectError::Limit(_)
            )) | BeforeChangePreviewError::Edit(EditError::Inspect(InspectError::Limit(_)))
        )
    )
}

fn cancelled(error: &BeforeChangeBackendError) -> bool {
    matches!(
        error,
        BeforeChangeBackendError::Native(
            NativeEditError::Inspect(InspectError::Cancelled)
                | NativeEditError::Snapshot(SnapshotError::Cancelled)
                | NativeEditError::Edit(EditError::Inspect(InspectError::Cancelled))
        ) | BeforeChangeBackendError::Preview(
            BeforeChangePreviewError::Preparation(BeforeEventGuardError::Inspect(
                InspectError::Cancelled
            )) | BeforeChangePreviewError::Edit(EditError::Inspect(InspectError::Cancelled))
        )
    )
}

#[test]
fn before_change_rich_measures_all_three_pools_and_cancels_at_final_poll() {
    let case = model::source(6, Recipe::SequentialUnionRows);
    let polls = Cell::new(0_u64);
    let mut measured = budget();
    typed(&case, false, &mut measured, &|| {
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
        false,
        &mut ResourceBudget::new(used[0], used[1], used[2]),
        &|| false,
    )
    .unwrap();
    assert_report(&copy, &case);
    for position in 0..resources.len() {
        let mut limits = used;
        limits[position] -= 1;
        let error = typed(
            &case,
            false,
            &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
            &|| false,
        )
        .err()
        .expect("one-low pool");
        assert!(limited(&error), "{error:?}");
    }
    let final_poll = polls.get();
    assert!(final_poll > 0);
    polls.set(0);
    let error = typed(&case, false, &mut budget(), &|| {
        polls.set(polls.get() + 1);
        polls.get() == final_poll
    })
    .err()
    .expect("final cancellation");
    assert!(cancelled(&error), "{error:?}");
    assert_eq!(polls.get(), final_poll);
}
