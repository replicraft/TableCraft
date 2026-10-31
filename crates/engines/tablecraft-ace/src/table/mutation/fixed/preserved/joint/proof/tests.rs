//! Causal checks use the independently frozen joint source/expected owner.
use super::super::super::{copy, joint_fixed_sources::model, source};
use super::{Observed, Original, prove, prove_original};
use crate::{
    ConstraintViolation, EditError, FixedColumnUpdates, FixedScalarUpdate, InspectError,
    NativeImage, WriteBlocker,
};
use model::{Cell, Column, Recipe, Refusal};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::FieldConstraintViolation;

const PAGE_BYTES: usize = 4096;
const OPAQUE_BYTE: usize = 31 * PAGE_BYTES + 37;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn requests(groups: &[model::Group]) -> Vec<Vec<FixedScalarUpdate>> {
    groups
        .iter()
        .map(|group| {
            group
                .updates
                .iter()
                .map(|update| {
                    FixedScalarUpdate::new(
                        update.row,
                        update.expected.value(),
                        update.replacement.value(),
                    )
                })
                .collect()
        })
        .collect()
}

fn groups<'a>(
    source: &'a [model::Group],
    updates: &'a [Vec<FixedScalarUpdate>],
) -> Vec<FixedColumnUpdates<'a>> {
    source
        .iter()
        .zip(updates)
        .map(|(group, updates)| FixedColumnUpdates::new(group.table, group.name, updates))
        .collect()
}

fn audit_groups(
    case: &model::Case,
    specifications: &[model::Group],
    observed: &[u8],
    count: usize,
) -> Result<(), EditError> {
    let updates = requests(specifications);
    let groups = groups(specifications, &updates);
    prove(
        &image(&case.before),
        &groups,
        observed,
        count,
        &mut budget(),
        &|| false,
    )
}

fn audit(case: &model::Case, observed: &[u8], count: usize) -> Result<(), EditError> {
    audit_groups(case, &case.groups, observed, count)
}

#[test]
fn joint_proof_accepts_nineteen_literal_images_both_orders_and_counts_root_union() {
    let mut positives = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let Some(expected) = &case.expected else {
            continue;
        };
        audit(&case, expected, case.changed).unwrap();
        let mut reversed = case.groups.clone();
        reversed.reverse();
        for group in &mut reversed {
            group.updates.reverse();
        }
        audit_groups(&case, &reversed, expected, case.changed).unwrap();
        assert_eq!(
            audit(&case, expected, case.changed + 1),
            Err(EditError::Preservation)
        );
        if case.changed != 0 {
            assert_eq!(audit(&case, expected, 0), Err(EditError::Preservation));
        }
        let assignment_count = case
            .groups
            .iter()
            .flat_map(|group| &group.updates)
            .filter(|update| update.expected != update.replacement)
            .count();
        if assignment_count != case.changed {
            assert_eq!(
                audit(&case, expected, assignment_count),
                Err(EditError::Preservation)
            );
        }
        positives += 1;
    }
    assert_eq!(positives, 19);
}

#[test]
fn joint_proof_rejects_missing_bits_and_whole_shared_mask_overwrite() {
    for (recipe, columns) in [
        (Recipe::SharedLowMask, vec![Column::Date, Column::Time]),
        (
            Recipe::SharedHighMask,
            vec![Column::Currency, Column::Yes, Column::No],
        ),
    ] {
        let case = model::source(6, recipe);
        let expected = case.expected.as_ref().unwrap();
        audit(&case, expected, 1).unwrap();
        let (_, shared_mask) = model::location(&case.before, columns[0], 0);
        for column in &columns {
            let (_, mask) = model::location(&case.before, *column, 0);
            assert_eq!(mask, shared_mask);
            let bit = 1 << (column.number() % 8);
            let mut missing = expected.clone();
            missing[mask] = (missing[mask] & !bit) | (case.before[mask] & bit);
            assert_eq!(audit(&case, &missing, 1), Err(EditError::Preservation));
        }
        let last = columns.last().unwrap();
        let bit = 1 << (last.number() % 8);
        let mut overwritten = expected.clone();
        overwritten[shared_mask] =
            (case.before[shared_mask] & !bit) | (expected[shared_mask] & bit);
        assert_ne!(overwritten, *expected);
        assert_eq!(audit(&case, &overwritten, 1), Err(EditError::Preservation));
    }
}

#[test]
fn joint_proof_rejects_date_alias_normalization_and_inactive_null_payload_changes() {
    let case = model::source(6, Recipe::DateAliases);
    let expected = case.expected.as_ref().unwrap();
    audit(&case, expected, 1).unwrap();
    for column in [Column::Date, Column::Time] {
        let (at, _) = model::location(&case.before, column, 0);
        let mut alias = expected.clone();
        alias[at..at + 8].copy_from_slice(&case.before[at..at + 8]);
        assert_eq!(audit(&case, &alias, 1), Err(EditError::Preservation));
    }

    let case = model::source(6, Recipe::InactiveNullNoops);
    let expected = case.expected.as_ref().unwrap();
    assert_eq!(expected, &case.before);
    let (at, _) = model::location(&case.before, Column::Date, 0);
    assert_eq!(&case.before[at..at + 8], &[255; 8]);
    let mut normalized = expected.clone();
    normalized[at..at + 8].fill(0);
    assert_eq!(audit(&case, &normalized, 0), Err(EditError::Preservation));

    let case = model::source(6, Recipe::SharedHighMask);
    let expected = case.expected.as_ref().unwrap();
    let (at, _) = model::location(&case.before, Column::Currency, 0);
    assert_eq!(&expected[at..at + 8], &case.before[at..at + 8]);
    let mut erased = expected.clone();
    erased[at..at + 8].fill(0);
    assert_eq!(audit(&case, &erased, 1), Err(EditError::Preservation));
}

fn cache_payload(bytes: &[u8]) -> usize {
    let (fixed, mask) = model::location(bytes, Column::Large, 0);
    let row_start = fixed - 2 - Column::Large.offset();
    let count_at = mask - 2;
    assert_eq!(
        u16::from_le_bytes(bytes[count_at..count_at + 2].try_into().unwrap()),
        5
    );
    let cached_offset_at = count_at - 2 * 5;
    let cached_offset = usize::from(u16::from_le_bytes(
        bytes[cached_offset_at..cached_offset_at + 2]
            .try_into()
            .unwrap(),
    ));
    let cached = row_start + cached_offset;
    assert_eq!(&bytes[cached..cached + 16], &[0; 16]);
    assert_eq!(&bytes[cached + 16..cached + 20], &4_u32.to_le_bytes());
    assert_eq!(&bytes[cached + 20..cached + 24], &2_i32.to_le_bytes());
    cached + 20
}

#[test]
fn joint_proof_rejects_adjacent_fixed_padding_cache_and_opaque_changes() {
    let case = model::source(6, Recipe::BooleanAndLarge);
    let expected = case.expected.as_ref().unwrap();
    let (payload, _) = model::location(&case.before, Column::Large, 0);
    assert_eq!(case.before[OPAQUE_BYTE], 0xc7);
    for offset in [payload - 1, cache_payload(&case.before), OPAQUE_BYTE] {
        let mut forged = expected.clone();
        forged[offset] ^= 1;
        assert_eq!(
            audit(&case, &forged, case.changed),
            Err(EditError::Preservation)
        );
    }
    let case = model::source(6, Recipe::Int32NullPadding);
    let expected = case.expected.as_ref().unwrap();
    for row in [0, 1] {
        let (payload, _) = model::location(&case.before, Column::LargeInt32, row);
        assert_eq!(
            &expected[payload + 4..payload + 8],
            &case.before[payload + 4..payload + 8]
        );
        let mut forged = expected.clone();
        forged[payload + 4] ^= 1;
        assert_eq!(audit(&case, &forged, 2), Err(EditError::Preservation));
    }
}

#[test]
fn joint_proof_binds_alternative_same_width_intent_and_all_original_refusals() {
    let case = model::source(6, Recipe::ThreePayloads);
    let expected = case.expected.as_ref().unwrap();
    let mut alternative = case.groups.clone();
    alternative[0].updates[0].replacement = Cell::Int64(-102);
    assert_eq!(
        audit_groups(&case, &alternative, expected, 1),
        Err(EditError::Preservation)
    );

    let mut refused = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let Some(refusal) = case.refusal else {
            continue;
        };
        if refusal == Refusal::Required {
            continue;
        }
        let expected_error = match refusal {
            Refusal::Conflict => EditError::Conflict,
            Refusal::IndexedColumn => EditError::Unsupported(WriteBlocker::IndexedColumn),
            Refusal::Column => EditError::Unsupported(WriteBlocker::Column),
            Refusal::Required => unreachable!(),
        };
        assert_eq!(audit(&case, &case.before, 0), Err(expected_error));
        refused += 1;
    }
    assert_eq!(refused, 7);
}

#[test]
fn joint_proof_validates_original_required_rule_before_observed_property_disable() {
    let case = model::source(6, Recipe::LateRequired);
    assert_eq!(case.refusal, Some(Refusal::Required));
    let (mut forged, changed) = model::reconstruct(&case.before, &case.groups);
    assert_eq!(changed, 1);
    let property = case.required_patch.unwrap();
    assert_eq!(case.before[property], 1);
    forged[property] = 0;
    assert_eq!(
        audit(&case, &forged, changed),
        Err(EditError::Inspect(InspectError::Constraint(
            ConstraintViolation::Field(FieldConstraintViolation::Required)
        )))
    );
}

#[test]
fn joint_proof_reuses_only_auditor_source_and_boolean_prefix_without_losing_bits() {
    let case = model::source(6, Recipe::BooleanAndLarge);
    let expected = case.expected.as_ref().unwrap();
    let image = image(&case.before);
    let original = source::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    let mut accumulated = copy(image.as_bytes(), &mut budget(), &|| false).unwrap();
    let (_, mask) = model::location(&case.before, Column::Yes, 0);
    accumulated[mask] |= 1 << (Column::Yes.number() % 8);
    let updates = requests(&case.groups);
    let groups = groups(&case.groups, &updates);
    prove_original(
        Original {
            image: &image,
            source: &original,
        },
        accumulated,
        &groups,
        Observed {
            bytes: expected,
            changed_rows: 1,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
}
