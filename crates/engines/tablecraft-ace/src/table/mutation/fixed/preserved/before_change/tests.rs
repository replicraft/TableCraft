//! Frozen original/request/XML images and independent physical proof controls.
use super::super::{
    joint_fixed_sources::model as fixed_model, rich_before_change_sources::expectations::model,
    source,
};
use super::proof::{self, Observed};
use crate::{
    Ace16BeforeChangePreview, BeforeChangePreviewError, BeforeEventGuardError,
    BeforeEventGuardSelector, BeforeGuardTraceKind, ConstraintViolation, EditError,
    FixedColumnUpdates, FixedScalarUpdate, InspectError, NativeImage, inspect_table_schema,
    table::mutation::guards::fields::batch::{self, Prepared, SelectedPrograms, SourceFacts},
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_schema::FieldConstraintViolation;

const PAGE_BYTES: usize = 4096;
const OPAQUE_BYTE: usize = 31 * PAGE_BYTES + 37;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn selectors(case: &model::Case) -> Vec<BeforeEventGuardSelector<'_>> {
    case.source
        .selection
        .iter()
        .map(|&index| BeforeEventGuardSelector::new(3, &case.source.properties[index].0))
        .collect()
}

fn image(
    bytes: &[u8],
    resources: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeImage, BeforeChangePreviewError> {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(7),
        resources,
        cancelled,
    )
    .map_err(BeforeEventGuardError::from)
    .map_err(BeforeChangePreviewError::from)
}

fn with_groups<T>(
    case: &model::Case,
    reverse_groups: bool,
    reverse_rows: bool,
    action: impl FnOnce(&[FixedColumnUpdates<'_>]) -> T,
) -> T {
    let mut specifications = case.groups.iter().collect::<Vec<_>>();
    if reverse_groups {
        specifications.reverse();
    }
    let updates = specifications
        .iter()
        .map(|group| {
            let mut rows = group
                .updates
                .iter()
                .map(|update| {
                    FixedScalarUpdate::new(
                        update.row,
                        update.expected.value(),
                        update.replacement.value(),
                    )
                })
                .collect::<Vec<_>>();
            if reverse_rows {
                rows.reverse();
            }
            rows
        })
        .collect::<Vec<_>>();
    let groups = specifications
        .iter()
        .zip(&updates)
        .map(|(group, updates)| FixedColumnUpdates::new(group.table, group.name, updates))
        .collect::<Vec<_>>();
    action(&groups)
}

fn run(
    case: &model::Case,
    reverse_groups: bool,
    reverse_rows: bool,
    resources: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16BeforeChangePreview, BeforeChangePreviewError> {
    with_groups(case, reverse_groups, reverse_rows, |groups| {
        image(&case.source.before, resources, cancelled)?.preview_ace16_before_change_batch_updates(
            "Items",
            &selectors(case),
            groups,
            resources,
            cancelled,
        )
    })
}

fn prepared<'image>(
    image: &'image NativeImage,
    case: &model::Case,
) -> Result<Prepared<'image>, BeforeChangePreviewError> {
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .map_err(BeforeEventGuardError::from)?;
    let selected =
        SelectedPrograms::capture(image, &schema, &selectors(case), &mut budget(), &|| false)?;
    let original =
        source::capture_before_change(image, "Items", &selected, &mut budget(), &|| false)?;
    with_groups(case, false, false, |groups| {
        batch::prepare(
            image,
            SourceFacts {
                table: &original.table,
                indexes: &original.indexes,
                dependencies: &original.dependencies,
            },
            selected,
            groups,
            &mut budget(),
            &|| false,
        )
        .map_err(BeforeChangePreviewError::from)
    })
}

fn audit(
    image: &NativeImage,
    case: &model::Case,
    prepared: &Prepared<'_>,
    bytes: &[u8],
    count: usize,
) -> Result<(), BeforeChangePreviewError> {
    with_groups(case, false, false, |groups| {
        proof::prove(
            image,
            "Items",
            &selectors(case),
            groups,
            Observed {
                prepared,
                bytes,
                changed_rows: count,
            },
            &mut budget(),
            &|| false,
        )
    })
}

#[test]
fn rich_before_change_literal_images_use_original_rhs_current_old_and_one_row_union() {
    let mut images = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let Some(expected) = &case.expected else {
            continue;
        };
        model::assert_complement(&case);
        for reverse_groups in [false, true] {
            for reverse_rows in [false, true] {
                let result = run(&case, reverse_groups, reverse_rows, &mut budget(), &|| {
                    false
                })
                .unwrap();
                assert_eq!(result.before_bytes(), case.source.before, "{recipe:?}");
                assert_eq!(result.candidate_bytes(), expected, "{recipe:?}");
                assert_eq!(result.changed_rows(), case.changed, "{recipe:?}");
                let rows = result
                    .trace()
                    .iter()
                    .filter_map(|entry| {
                        if entry.kind() == BeforeGuardTraceKind::Begin {
                            entry.row()
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                assert_eq!(rows, case.event_rows, "{recipe:?}");
                let assignments = result
                    .assignments()
                    .iter()
                    .map(|assignment| (assignment.ordinal(), assignment.column()))
                    .collect::<Vec<_>>();
                assert_eq!(
                    assignments,
                    case.assignments
                        .iter()
                        .map(|assignment| (assignment.row, assignment.column))
                        .collect::<Vec<_>>(),
                    "{recipe:?}"
                );
                for assignment in result.assignments() {
                    assert!(assignment.program() < result.programs().len());
                    assert!(assignment.source_span().start() < assignment.source_span().end());
                    assert!(assignment.trace_position() <= result.trace().len());
                }
                assert!(
                    result
                        .assignments()
                        .windows(2)
                        .all(|pair| pair[0].trace_position() <= pair[1].trace_position())
                );
                assert_eq!(result.programs().len(), case.source.selection.len());
                for (program, &selected) in result.programs().iter().zip(&case.source.selection) {
                    assert_eq!(program.group(), 3);
                    assert_eq!(program.name(), case.source.properties[selected].0);
                    let raw = program.native_property_bytes();
                    assert!(!raw.is_empty());
                    assert!(
                        case.source
                            .extra
                            .windows(raw.len())
                            .any(|bytes| bytes == raw)
                    );
                }
                if case.changed == 0 {
                    assert_eq!(result.candidate_bytes(), case.source.before);
                }
                images += 1;
            }
        }
    }
    assert_eq!(images, 60);
    let restored = model::source(6, model::Recipe::SequentialRestoresOriginal);
    let result = run(&restored, false, false, &mut budget(), &|| false).unwrap();
    assert_eq!(result.changed_rows(), 0);
    assert_eq!(result.candidate_bytes(), restored.source.before);
    assert!(!result.assignments().is_empty());
    assert!(!restored.event_rows.is_empty());
}

#[test]
fn rich_before_change_rejection_never_returns_a_successful_prefix() {
    let case = model::source(6, model::Recipe::LateRaise);
    assert!(case.expected.is_none());
    assert_eq!(case.event_rows, [0, 1]);
    assert!(
        case.assignments
            .iter()
            .any(|assignment| assignment.row == 0)
    );
    for reverse_groups in [false, true] {
        for reverse_rows in [false, true] {
            match run(&case, reverse_groups, reverse_rows, &mut budget(), &|| {
                false
            }) {
                Err(BeforeChangePreviewError::Rejected(rejection)) => {
                    assert_eq!(rejection.number(), 17)
                }
                _ => {
                    panic!("late RaiseError must discard all rows, assignments and candidate bytes")
                }
            }
        }
    }
}

#[test]
fn rich_before_change_all_programs_and_targets_qualify_for_empty_rows() {
    for recipe in [
        model::Recipe::DormantIndexedTarget,
        model::Recipe::DormantDependentTarget,
        model::Recipe::CounterTarget,
        model::Recipe::OpaqueFieldRead,
        model::Recipe::WideFieldRead,
        model::Recipe::ComplexCurrentRead,
        model::Recipe::ComplexOldRead,
        model::Recipe::BodyPresent,
        model::Recipe::ModulePresent,
        model::Recipe::OpaqueExtra,
        model::Recipe::UnselectedProperty,
        model::Recipe::ChildPresent,
    ] {
        let mut case = model::source(6, recipe);
        for group in &mut case.groups {
            group.updates.clear();
        }
        assert!(
            run(&case, false, false, &mut budget(), &|| false).is_err(),
            "{recipe:?}"
        );
    }
    let counter = model::source(6, model::Recipe::CounterOperand);
    let positive = run(&counter, false, false, &mut budget(), &|| false).unwrap();
    assert_eq!(
        positive.candidate_bytes(),
        counter.expected.as_ref().unwrap()
    );
    let empty = model::source(6, model::Recipe::EntirelyEmpty);
    assert!(!empty.groups.is_empty());
    assert!(empty.groups.iter().all(|group| group.updates.is_empty()));
    let result = run(&empty, false, false, &mut budget(), &|| false).unwrap();
    assert_eq!(result.candidate_bytes(), empty.source.before);
    assert_eq!(result.changed_rows(), 0);
    assert!(result.assignments().is_empty());
}

#[test]
fn rich_before_change_cumulative_exact_pools_one_below_and_final_cancel() {
    let case = model::source(6, model::Recipe::SequentialUnionRows);
    let mut measured = budget();
    let polls = Cell::new(0);
    let result = run(&case, false, false, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(result.candidate_bytes(), case.expected.as_ref().unwrap());
    let used = [Resource::Bytes, Resource::Rows, Resource::Steps]
        .map(|resource| budget().remaining(resource) - measured.remaining(resource));
    assert!(used.iter().all(|amount| *amount > 0));
    assert_eq!(
        run(
            &case,
            false,
            false,
            &mut ResourceBudget::new(used[0], used[1], used[2]),
            &|| false
        )
        .unwrap()
        .candidate_bytes(),
        case.expected.as_ref().unwrap()
    );
    for position in 0..used.len() {
        let mut limits = used;
        limits[position] -= 1;
        assert!(matches!(
            run(
                &case,
                false,
                false,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false
            ),
            Err(BeforeChangePreviewError::Preparation(
                BeforeEventGuardError::Inspect(InspectError::Limit(_))
            )) | Err(BeforeChangePreviewError::Edit(EditError::Inspect(
                InspectError::Limit(_)
            )))
        ));
    }
    let last = polls.get();
    polls.set(0);
    assert!(matches!(
        run(&case, false, false, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == last
        }),
        Err(BeforeChangePreviewError::Preparation(
            BeforeEventGuardError::Inspect(InspectError::Cancelled)
        )) | Err(BeforeChangePreviewError::Edit(EditError::Inspect(
            InspectError::Cancelled
        )))
    ));
    assert_eq!(polls.get(), last);
}

#[test]
fn rich_before_change_seal_rejects_equal_bytes_from_a_different_image() {
    let case = model::source(6, model::Recipe::SequentialSeeded);
    let first = image(&case.source.before, &mut budget(), &|| false).unwrap();
    let second = image(&case.source.before, &mut budget(), &|| false).unwrap();
    let schema = inspect_table_schema(
        &mut first.reader(),
        first.catalog().catalog().scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let selected =
        SelectedPrograms::capture(&first, &schema, &selectors(&case), &mut budget(), &|| false)
            .unwrap();
    assert!(matches!(
        source::capture_before_change(&second, "Items", &selected, &mut budget(), &|| false),
        Err(EditError::Preservation)
    ));
    let preparation = prepared(&first, &case).unwrap();
    assert!(
        audit(
            &second,
            &case,
            &preparation,
            case.expected.as_ref().unwrap(),
            case.changed
        )
        .is_err()
    );
}

#[test]
fn rich_before_change_proof_rejects_forged_bytes_counts_and_alternate_intent() {
    let case = model::source(6, model::Recipe::SequentialSeeded);
    let original = image(&case.source.before, &mut budget(), &|| false).unwrap();
    let preparation = prepared(&original, &case).unwrap();
    let expected = case.expected.as_ref().unwrap();
    audit(&original, &case, &preparation, expected, case.changed).unwrap();
    let (payload, mask) =
        fixed_model::location(&case.source.before, fixed_model::Column::LargeInt32, 0);
    let extra = case
        .source
        .before
        .windows(case.source.extra.len())
        .position(|bytes| bytes == case.source.extra)
        .unwrap();
    for at in [payload, mask, OPAQUE_BYTE, extra] {
        let mut forged = expected.clone();
        forged[at] ^= 1;
        assert!(matches!(
            audit(&original, &case, &preparation, &forged, case.changed),
            Err(BeforeChangePreviewError::Edit(EditError::Preservation))
        ));
    }
    assert!(matches!(
        audit(&original, &case, &preparation, expected, case.changed + 1),
        Err(BeforeChangePreviewError::Edit(EditError::Preservation))
    ));
    let mut alternative = model::source(6, model::Recipe::SequentialSeeded);
    let update = alternative
        .groups
        .iter_mut()
        .find(|group| group.name == "Large_Number")
        .unwrap()
        .updates
        .first_mut()
        .unwrap();
    update.replacement = fixed_model::Cell::Int32(11);
    assert!(matches!(
        audit(
            &original,
            &alternative,
            &preparation,
            expected,
            case.changed
        ),
        Err(BeforeChangePreviewError::Preparation(
            BeforeEventGuardError::Preservation
        ))
    ));
}

#[test]
fn rich_before_change_candidate_rebinding_requires_exact_well_formed_extra() {
    let case = model::source(6, model::Recipe::SequentialSeeded);
    let original = image(&case.source.before, &mut budget(), &|| false).unwrap();
    let prepared = prepared(&original, &case).unwrap();
    let expected = case.expected.as_ref().unwrap();
    let candidate = image(expected, &mut budget(), &|| false).unwrap();
    let schema = inspect_table_schema(
        &mut candidate.reader(),
        candidate.catalog().catalog().scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let selected = prepared
        .admission()
        .rebind_candidate(&candidate, &schema, &mut budget(), &|| false)
        .unwrap();
    source::capture_before_change(&candidate, "Items", &selected, &mut budget(), &|| false)
        .unwrap();

    let extra = expected
        .windows(case.source.extra.len())
        .position(|bytes| bytes == case.source.extra)
        .unwrap();
    let plus = case
        .source
        .extra
        .windows(2)
        .position(|bytes| bytes == [b'+', 0])
        .unwrap();
    let mut changed = expected.clone();
    // Changing an operator preserves UTF16/XML/MR2 framing and all catalog roles.
    // Complete fresh inspection succeeds; original raw program binding must fail.
    assert_eq!(changed[extra + plus], b'+');
    changed[extra + plus] = b'-';
    let candidate = image(&changed, &mut budget(), &|| false).unwrap();
    let schema = inspect_table_schema(
        &mut candidate.reader(),
        candidate.catalog().catalog().scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        prepared
            .admission()
            .rebind_candidate(&candidate, &schema, &mut budget(), &|| false)
            .err(),
        Some(BeforeEventGuardError::Preservation)
    );
}

#[test]
fn rich_before_change_original_intents_stay_authoritative_before_macro_overwrite() {
    for mismatch in [fixed_model::Cell::Int32(999), fixed_model::Cell::Null] {
        let mut case = model::source(6, model::Recipe::SequentialSeeded);
        let group = case
            .groups
            .iter_mut()
            .find(|group| group.name == "Large_Number")
            .unwrap();
        group.updates[0].expected = mismatch;
        assert!(matches!(
            run(&case, false, false, &mut budget(), &|| false),
            Err(BeforeChangePreviewError::Preparation(
                BeforeEventGuardError::Intent(EditError::Conflict)
            ))
        ));
    }
    let mut wrong_carrier = model::source(6, model::Recipe::SequentialSeeded);
    wrong_carrier.groups[0].updates[0].expected = fixed_model::Cell::Int64(1000);
    assert!(run(&wrong_carrier, false, false, &mut budget(), &|| false).is_err());
    let mut duplicate = model::source(6, model::Recipe::SequentialSeeded);
    let assignment = duplicate.groups[0].updates[0];
    duplicate.groups[0].updates.push(assignment);
    assert!(run(&duplicate, false, false, &mut budget(), &|| false).is_err());
    let mut absent = model::source(6, model::Recipe::SequentialSeeded);
    absent.groups[0].updates[0].row = 3;
    assert!(run(&absent, false, false, &mut budget(), &|| false).is_err());
    let mut repeated_target = model::source(6, model::Recipe::SequentialSeeded);
    repeated_target
        .groups
        .push(repeated_target.groups[0].clone());
    assert!(run(&repeated_target, false, false, &mut budget(), &|| false).is_err());

    for target in ["Number", "ID", "Attachment_Field", "Calculated", "Unknown"] {
        let mut case = model::source(6, model::Recipe::SequentialSeeded);
        let mut empty = case.groups[0].clone();
        empty.name = target;
        empty.updates.clear();
        case.groups.push(empty);
        assert!(
            run(&case, false, false, &mut budget(), &|| false).is_err(),
            "{target}"
        );
    }
}

#[test]
fn rich_before_change_required_is_checked_after_the_complete_transformation() {
    let repaired = model::source(6, model::Recipe::RequiredRepair);
    let result = run(&repaired, false, false, &mut budget(), &|| false).unwrap();
    assert_eq!(
        result.candidate_bytes(),
        repaired.expected.as_ref().unwrap()
    );
    assert_eq!(result.changed_rows(), 1);
    assert!(!result.assignments().is_empty());

    let unrepaired = model::source(6, model::Recipe::RequiredUnrepaired);
    assert!(unrepaired.expected.is_none());
    assert!(unrepaired.assignments.is_empty());
    let original = image(&unrepaired.source.before, &mut budget(), &|| false).unwrap();
    let preparation = prepared(&original, &unrepaired).unwrap();
    assert!(preparation.rejection().is_none());
    // Independently author a physically self-consistent final image. There are
    // no SetField assignments in this guard-only case, so the original fixed
    // request constructor is the complete expected physical transformation.
    let (invalid, changed) =
        fixed_model::reconstruct(&unrepaired.source.before, &unrepaired.groups);
    assert_ne!(invalid, unrepaired.source.before);
    assert!(matches!(
        audit(&original, &unrepaired, &preparation, &invalid, changed),
        Err(BeforeChangePreviewError::Edit(EditError::Inspect(
            InspectError::Constraint(ConstraintViolation::Field(
                FieldConstraintViolation::Required
            ))
        )))
    ));
    for reverse in [false, true] {
        assert!(matches!(
            run(&unrepaired, reverse, reverse, &mut budget(), &|| false),
            Err(BeforeChangePreviewError::Edit(EditError::Inspect(
                InspectError::Constraint(ConstraintViolation::Field(
                    FieldConstraintViolation::Required
                ))
            )))
        ));
    }
}

#[test]
fn rich_before_change_header_five_profile_refusals_are_preserved() {
    for recipe in model::RECIPES {
        let case = model::source(5, recipe);
        assert!(case.expected.is_none());
        // Role/selection failures may precede the unsupported extended sibling
        // admission; do not replace those concrete errors with one blanket code.
        assert!(
            run(&case, false, false, &mut budget(), &|| false).is_err(),
            "{recipe:?}"
        );
    }
}
