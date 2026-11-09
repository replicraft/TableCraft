//! Distinct consuming proof with independently retained SDK/raw complete images.
use super::*;
use crate::FieldAdditionAfterOffError;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::mixed::after_off::{self as raw, history},
    support::bytes::hex,
};

const SCOPE: CatalogScope = CatalogScope::new(1199);
const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn text() -> FieldAdditionTarget {
    FieldAdditionTarget::ShortText {
        max_utf16_units: 255,
        unicode_compression: true,
    }
}

#[test]
fn four_complete_candidates_match_independent_second_add_sdk_images() {
    for (case, number_pin, text_pin) in [
        (
            history::SourceCase::NumberThenNumber,
            "cf07d4ba065af7f694c263b3344a01ad6fd6a8f4ebe1e073088626ed2541919f",
            "020379bbab66648cb5f2b580f8cc5e903132d451483066dea94296bb967674b7",
        ),
        (
            history::SourceCase::MixedThenText,
            "a28268a3acb88767749b3d640e1dc2b072ff0fda515ccfc18735be113c337bd3",
            "275af21a3140ab69a320b7b2a86ef79207443d67777d55381f742d0cd6dab04a",
        ),
    ] {
        let before = history::source(case);
        for (column, target, expected, pin) in [
            (
                "SecondNumber",
                FieldAdditionTarget::NumberLongZero,
                raw::append_number(&before, "SecondNumber"),
                number_pin,
            ),
            (
                "SecondLabel",
                text(),
                raw::append_text(&before, "SecondLabel"),
                text_pin,
            ),
        ] {
            let candidate = image(&before)
                .prepare_created_add_field_after_off_history_candidate(
                    "Table1",
                    column,
                    target,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(candidate.before_bytes(), before);
            assert_eq!(candidate.after_bytes(), expected);
            assert_eq!(hex(&Sha256::digest(candidate.after_bytes())), pin);
            assert_eq!(candidate.table(), "Table1");
            assert_eq!(candidate.column(), column);
            assert_eq!(candidate.target(), target);
            let count = u16::from_le_bytes(before[7 * PAGE + 45..7 * PAGE + 47].try_into().unwrap())
                as usize;
            assert_eq!(candidate.column_count(), count + 1);
            assert_eq!(
                &candidate.after_bytes()[3 * PAGE..4 * PAGE],
                &before[3 * PAGE..4 * PAGE]
            );
            assert_eq!(
                &candidate.after_bytes()[9 * PAGE..10 * PAGE],
                &before[9 * PAGE..10 * PAGE]
            );
            image(candidate.after_bytes())
                .check_created_field_addition_after_off_history_source(
                    "Table1",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert!(
                image(&before)
                    .prepare_created_add_field_after_off_candidate(
                        "Table1",
                        column,
                        target,
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
        }
    }
}

#[test]
fn raw_third_and_fourth_additions_use_fresh_complete_history_proofs() {
    let first = history::source(history::SourceCase::NumberThenNumber);
    let second = raw::append_text(&first, "SecondLabel");
    let third = raw::append_number(&second, "ThirdNumber");
    let fourth = raw::append_text(&third, "FourthLabel");
    for (before, column, target, expected) in [
        (
            second,
            "ThirdNumber",
            FieldAdditionTarget::NumberLongZero,
            third.clone(),
        ),
        (third, "FourthLabel", text(), fourth),
    ] {
        let actual = image(&before)
            .prepare_created_add_field_after_off_history_candidate(
                "Table1",
                column,
                target,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(actual.after_bytes(), expected);
    }
    // Only independent raw fixtures qualify these later images; no SDK claim.
}

#[test]
fn initial_off_source_unknown_target_and_unqualified_namespace_do_not_migrate() {
    let initial = raw::source(raw::SourceCase::Number);
    assert!(matches!(
        image(&initial).prepare_created_add_field_after_off_history_candidate(
            "Table1",
            "Next",
            FieldAdditionTarget::NumberLongZero,
            &mut budget(),
            &|| false
        ),
        Err(Error::Source)
    ));
    let before = history::source(history::SourceCase::NumberThenNumber);
    for target in [
        FieldAdditionTarget::ShortText {
            max_utf16_units: 80,
            unicode_compression: true,
        },
        FieldAdditionTarget::ShortText {
            max_utf16_units: 255,
            unicode_compression: false,
        },
    ] {
        assert!(matches!(
            image(&before).prepare_created_add_field_after_off_history_candidate(
                "Table1",
                "Next",
                target,
                &mut budget(),
                &|| false
            ),
            Err(Error::Field(FieldAdditionAfterOffError::Target))
        ));
    }
    for column in [
        "Σ",
        "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklm",
    ] {
        assert!(matches!(
            image(&before).prepare_created_add_field_after_off_history_candidate(
                "Table1",
                column,
                FieldAdditionTarget::NumberLongZero,
                &mut budget(),
                &|| false
            ),
            Err(Error::Field(FieldAdditionAfterOffError::Namespace))
        ));
    }
}

#[test]
fn source_tail_and_independent_controls_are_preserved() {
    let before = history::source(history::SourceCase::NumberThenNumber);
    let mut changed = before.clone();
    changed[7 * PAGE + 3000] ^= 1;
    assert!(matches!(
        image(&changed).prepare_created_add_field_after_off_history_candidate(
            "Table1",
            "Next",
            FieldAdditionTarget::NumberLongZero,
            &mut budget(),
            &|| false
        ),
        Err(Error::Preservation)
    ));
    assert!(matches!(
        image(&before).prepare_created_add_field_after_off_history_candidate(
            "Table1",
            "Next",
            FieldAdditionTarget::NumberLongZero,
            &mut budget(),
            &|| true
        ),
        Err(Error::Field(FieldAdditionAfterOffError::Inspect(
            crate::InspectError::Cancelled
        )))
    ));
    assert!(
        image(&before)
            .prepare_created_add_field_after_off_history_candidate(
                "Table1",
                "Next",
                FieldAdditionTarget::NumberLongZero,
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
}
