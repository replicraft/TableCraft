//! Independent complete-image authority, including raw representation no-ops.
use super::super::super::short_text_sources::{model, zero_length_source};
use super::{Observed, prove};
use crate::{
    ConstraintViolation, EditError, InspectError, NativeImage, SlotScalarUpdate,
    inspect_table_properties,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::FieldConstraintViolation;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 10_000_000)
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

fn request(case: &model::Case) -> SlotScalarUpdate {
    SlotScalarUpdate::new(
        case.framing.row,
        case.original
            .map_or(Value::Null, |value| Value::Text(value.to_owned())),
        case.replacement
            .map_or(Value::Null, |value| Value::Text(value.to_owned())),
    )
}

#[test]
fn preserved_text_proof_rebuilds_all_literal_images_and_rejects_wrong_counts() {
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let Some(expected) = &case.expected else {
            continue;
        };
        let original = image(&case.before);
        let requests = [request(&case)];
        let count = usize::from(case.original != case.replacement);
        prove(
            &original,
            "Items",
            "Short_Text",
            &requests,
            Observed {
                bytes: expected,
                changed_rows: count,
            },
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            prove(
                &original,
                "Items",
                "Short_Text",
                &requests,
                Observed {
                    bytes: expected,
                    changed_rows: count + 1
                },
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Preservation)
        );
    }
}

#[test]
fn preserved_text_proof_rejects_neighbor_mask_cache_and_opaque_page_forgeries() {
    let case = model::source(6, model::Recipe::ShorterNarrow);
    let expected = case.expected.as_ref().unwrap();
    let original = image(&case.before);
    let requests = [request(&case)];
    // The fifth variable payload is the existing stored calculation. Resolve
    // its relocated start from the independent expected footer, not a stage.
    let offset_word = case.framing.absolute + case.framing.count_at - 5 * 2;
    let cache = case.framing.absolute
        + usize::from(u16::from_le_bytes(
            expected[offset_word..offset_word + 2].try_into().unwrap(),
        ));
    for offset in [
        case.framing.absolute + case.framing.mask_at,
        cache,
        31 * 4096,
    ] {
        let mut forged = expected.clone();
        forged[offset] ^= 1;
        assert_eq!(
            prove(
                &original,
                "Items",
                "Short_Text",
                &requests,
                Observed {
                    bytes: &forged,
                    changed_rows: 1
                },
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Preservation)
        );
    }
}

#[test]
fn preserved_text_noop_keeps_inactive_bytes_and_cannot_authorize_other_requests() {
    let case = model::source(6, model::Recipe::NullNoop);
    let original = image(&case.before);
    let requests = [request(&case)];
    let mut forged = case.expected.as_ref().unwrap().clone();
    forged[case.framing.absolute + case.framing.offsets[0]] ^= 1;
    assert_eq!(
        prove(
            &original,
            "Items",
            "Short_Text",
            &requests,
            Observed {
                bytes: &forged,
                changed_rows: 0
            },
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Preservation)
    );

    let requested = model::source(6, model::Recipe::ShorterNarrow);
    let alternative = model::source(6, model::Recipe::ShorterUnicode);
    assert_eq!(requested.before, alternative.before);
    let requests = [request(&requested)];
    assert_eq!(
        prove(
            &image(&requested.before),
            "Items",
            "Short_Text",
            &requests,
            Observed {
                bytes: alternative.expected.as_ref().unwrap(),
                changed_rows: 1
            },
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Preservation)
    );
}

#[test]
fn preserved_text_proof_checks_rules_on_its_own_reconstructed_image() {
    let case = model::source(6, model::Recipe::Empty);
    let properties = inspect_table_properties(
        &mut Cursor::new(&case.before),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let (before, property_offset) =
        zero_length_source::disallow_empty(&case.before, properties.raw_bytes());
    let mut invalid = case.expected.as_ref().unwrap().clone();
    invalid[property_offset] = 0;
    let requests = [request(&case)];

    assert_eq!(
        prove(
            &image(&before),
            "Items",
            "Short_Text",
            &requests,
            Observed {
                bytes: &invalid,
                changed_rows: 1,
            },
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Inspect(InspectError::Constraint(
            ConstraintViolation::Field(FieldConstraintViolation::EmptyText)
        )))
    );
}
