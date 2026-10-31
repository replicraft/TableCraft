//! Literal successes precede independent representation and constraint forgeries.
use super::super::super::fixed_scalar_sources::model::{self, Recipe, Target};
use super::super::super::{copy, source};
use super::{Observed, Original, prove, prove_bound};
use crate::{
    ConstraintViolation, EditError, FixedColumnUpdates, FixedScalarUpdate, InspectError,
    NativeImage, WriteBlocker,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::FieldConstraintViolation;

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

fn requests(case: &model::Case) -> Vec<FixedScalarUpdate> {
    (0..case.requested_rows)
        .map(|row| {
            FixedScalarUpdate::new(
                row,
                case.target.value(case.initial[row]),
                case.target.value(case.replacement[row]),
            )
        })
        .collect()
}

fn audit(case: &model::Case, bytes: &[u8], count: usize) -> Result<(), EditError> {
    let requests = requests(case);
    let groups = [FixedColumnUpdates::new(
        "Items",
        case.target.name(),
        &requests,
    )];
    prove(
        &image(&case.before),
        &groups,
        bytes,
        count,
        &mut budget(),
        &|| false,
    )
}

#[test]
fn scalar_proof_accepts_literal_images_and_rejects_wrong_counts() {
    let mut checked = 0;
    for (target, recipe) in model::cases() {
        let case = model::source(6, target, recipe);
        let Some(expected) = &case.expected else {
            continue;
        };
        audit(&case, expected, case.changed).unwrap();
        assert_eq!(
            audit(&case, expected, case.changed + 1),
            Err(EditError::Preservation)
        );
        checked += 1;
    }
    assert_eq!(checked, 35);
}

#[test]
fn scalar_proof_rejects_date_alias_and_inactive_null_representations() {
    for recipe in [Recipe::SignedZero, Recipe::TemporalAlias] {
        let case = model::source(6, Target::Date, recipe);
        let expected = case.expected.as_ref().unwrap();
        audit(&case, expected, 1).unwrap();
        // The original DATE is temporally equal to the requested result, but
        // remains a different carrier representation and cannot replace it.
        assert_eq!(audit(&case, &case.before, 1), Err(EditError::Preservation));
    }
    let case = model::source(6, Target::Date, Recipe::NullNoop);
    let expected = case.expected.as_ref().unwrap();
    audit(&case, expected, 0).unwrap();
    let (payload, _) = model::location(&case.before, case.target, 0);
    assert_eq!(&case.before[payload..payload + 8], &[255; 8]);
    let mut normalized = expected.clone();
    normalized[payload..payload + 8].fill(0);
    assert_eq!(audit(&case, &normalized, 0), Err(EditError::Preservation));
}

#[test]
fn scalar_proof_rejects_mask_adjacent_and_opaque_changes_and_wrong_carriers() {
    let case = model::source(6, Target::Currency, Recipe::NullToSameBytes);
    let expected = case.expected.as_ref().unwrap();
    audit(&case, expected, 1).unwrap();
    let (payload, mask) = model::location(&case.before, case.target, 0);
    assert_eq!(
        &case.before[payload..payload + 8],
        &expected[payload..payload + 8]
    );
    for offset in [mask, payload - 1, 31 * 4096] {
        let mut forged = expected.clone();
        forged[offset] ^= if offset == mask {
            1 << (case.target.number() % 8)
        } else {
            1
        };
        assert_eq!(audit(&case, &forged, 1), Err(EditError::Preservation));
    }
    let case = model::source(6, Target::Date, Recipe::ValueNoop);
    let before = image(&case.before);
    for wrong in [Value::Int64(0), Value::Currency(Currency::from_scaled(0))] {
        let requests = [FixedScalarUpdate::new(0, wrong.clone(), wrong)];
        let groups = [FixedColumnUpdates::new(
            "Items",
            case.target.name(),
            &requests,
        )];
        assert_eq!(
            prove(&before, &groups, &case.before, 0, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        );
    }
}

#[test]
fn scalar_proof_checks_required_on_its_independently_reconstructed_image() {
    for target in model::TARGETS {
        let case = model::source(6, target, Recipe::RequiredRefusal);
        assert!(case.initial.iter().all(Option::is_some));
        let (_, mask) = model::location(&case.before, target, 0);
        let mut forged = case.before.clone();
        forged[mask] &= !(1 << (target.number() % 8));
        // A forged property disable would make this observed NULL legal if
        // the proof validated observed bytes instead of its own reconstruction.
        forged[case.required_patch.unwrap()] = 0;
        assert_eq!(
            audit(&case, &forged, 1),
            Err(EditError::Inspect(InspectError::Constraint(
                ConstraintViolation::Field(FieldConstraintViolation::Required)
            )))
        );
    }
}

#[test]
fn scalar_proof_binds_selected_metadata_and_original_request() {
    let case = model::source(6, Target::LargeNumber, Recipe::Change);
    let expected = case.expected.as_ref().unwrap();
    audit(&case, expected, 1).unwrap();
    let image = image(&case.before);
    let original = source::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    let other = source::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    let requests = requests(&case);
    let groups = [FixedColumnUpdates::new(
        "Items",
        case.target.name(),
        &requests,
    )];
    let foreign_selected = &other.table.schema().columns()[case.target.number()];
    assert_eq!(
        prove_bound(
            Original {
                image: &image,
                source: &original
            },
            copy(image.as_bytes(), &mut budget(), &|| false).unwrap(),
            &groups,
            foreign_selected,
            Observed {
                bytes: expected,
                changed_rows: 1
            },
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Preservation)
    );
    let alternative = [FixedScalarUpdate::new(
        0,
        case.target.value(case.initial[0]),
        Value::Int64(-102),
    )];
    let groups = [FixedColumnUpdates::new(
        "Items",
        case.target.name(),
        &alternative,
    )];
    assert_eq!(
        prove(&image, &groups, expected, 1, &mut budget(), &|| false),
        Err(EditError::Preservation)
    );
}
