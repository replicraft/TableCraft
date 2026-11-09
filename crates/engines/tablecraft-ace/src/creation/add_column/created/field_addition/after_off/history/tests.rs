//! Distinct chronological proof; old settled/candidate/wire grammar stays exact.
use super::*;
use crate::format::reader::row_range;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::{
        Source as NumberSource,
        mixed::after_off::{self as raw, history},
        repeated,
    },
    canonical_database_options,
};

const SCOPE: CatalogScope = CatalogScope::new(1198);
const PROPERTY_COUNT_AT: usize = 12;
const FIRST_DICTIONARY_NAME_BYTE: usize = 12;
const USER_DEFINITION_PAGE: usize = 7;
const UNINTERPRETED_DEFINITION_TAIL: usize = 3000;
const MAP_PAGE: usize = 3;
const UNINTERPRETED_MAP_GAP: usize = 3000;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn check_source(bytes: &[u8]) -> Result<(), FieldAdditionAfterOffHistoryError> {
    image(bytes).check_created_field_addition_after_off_history_source(
        "Table1",
        &mut budget(),
        &|| false,
    )
}

#[test]
fn four_first_event_images_qualify_without_widening_old_source_languages() {
    for case in [raw::SourceCase::Number, raw::SourceCase::Mixed] {
        let initial = raw::source(case);
        assert!(matches!(
            check_source(&initial),
            Err(FieldAdditionAfterOffHistoryError::Source)
        ));
        for current in [
            raw::append_number(&initial, "AfterOff"),
            raw::append_text(&initial, "LabelOff"),
        ] {
            check_source(&current).unwrap();
            assert!(
                image(&current)
                    .check_field_history_name_auto_correct_off_source(&mut budget(), &|| false)
                    .is_err()
            );
            assert!(
                image(&current)
                    .check_created_field_addition_source("Table1", &mut budget(), &|| false)
                    .is_err()
            );
            assert!(
                image(&current)
                    .prepare_created_add_field_after_off_candidate(
                        "Table1",
                        "Next",
                        FieldAdditionTarget::NumberLongZero,
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
        }
    }
}

#[test]
fn four_second_event_images_replay_every_post_db_snapshot_in_order() {
    for case in [
        history::SourceCase::NumberThenNumber,
        history::SourceCase::MixedThenText,
    ] {
        let before = history::source(case);
        for current in [
            raw::append_number(&before, "SecondNumber"),
            raw::append_text(&before, "SecondLabel"),
        ] {
            let original = current.clone();
            check_source(&current).unwrap();
            assert_eq!(current, original);
        }
    }
}

#[test]
fn additional_raw_only_mixed_history_is_bounded_by_real_native_replay() {
    let first = history::source(history::SourceCase::NumberThenNumber);
    let second = raw::append_text(&first, "SecondLabel");
    let third = raw::append_number(&second, "ThirdNumber");
    // This third-event image is an independent raw model, without an SDK Save claim.
    check_source(&third).unwrap();
}

#[test]
fn noncanonical_dictionary_refuses_and_erased_definition_tail_requires_full_replay() {
    let before = history::source(history::SourceCase::NumberThenNumber);
    let page =
        &before[PROPERTY_PAGE as usize * PAGE_BYTES..(PROPERTY_PAGE as usize + 1) * PAGE_BYTES];
    let count = usize::from(u16::from_le_bytes(
        page[PROPERTY_COUNT_AT..PROPERTY_COUNT_AT + 2]
            .try_into()
            .unwrap(),
    ));
    let (_, start, _) = row_range(page, count - 1).unwrap();
    let name = PROPERTY_PAGE as usize * PAGE_BYTES + start + FIRST_DICTIONARY_NAME_BYTE;
    assert_eq!(before[name], b'R');
    // Existing declaration capture requires exact canonical property keys.
    // This refusal must stay at that source guard rather than be normalized.
    let mut dictionary = before.clone();
    dictionary[name] = b'r';
    let observed =
        inspect_database_properties(&mut Cursor::new(&dictionary), SCOPE, &mut budget(), &|| {
            false
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        observed
            .name_auto_correct(&mut budget(), &|| false)
            .unwrap(),
        DatabaseNameAutoCorrect::ExplicitOff
    );
    assert!(matches!(
        check_source(&dictionary),
        Err(FieldAdditionAfterOffHistoryError::Field(
            FieldAdditionAfterOffError::Field(FieldAdditionError::Source)
        ))
    ));

    let mut tail = before;
    tail[USER_DEFINITION_PAGE * PAGE_BYTES + UNINTERPRETED_DEFINITION_TAIL] ^= 1;
    assert!(matches!(
        check_source(&tail),
        Err(FieldAdditionAfterOffHistoryError::Preservation)
    ));
}

#[test]
fn retained_pre_db_snapshots_and_maps_are_not_normalized_into_authority() {
    let before = history::source(history::SourceCase::NumberThenNumber);
    let page =
        &before[PROPERTY_PAGE as usize * PAGE_BYTES..(PROPERTY_PAGE as usize + 1) * PAGE_BYTES];
    let (_, start, _) = row_range(page, 0).unwrap();
    let mut changed = before.clone();
    changed[PROPERTY_PAGE as usize * PAGE_BYTES + start + FIRST_DICTIONARY_NAME_BYTE] ^= 1;
    assert!(check_source(&changed).is_err());
    let mut maps = before;
    maps[MAP_PAGE * PAGE_BYTES + UNINTERPRETED_MAP_GAP] ^= 1;
    assert!(check_source(&maps).is_err());
}

#[test]
fn one_pool_and_cancellation_are_retained_before_inverse_or_replay_work() {
    let before = history::source(history::SourceCase::NumberThenNumber);
    assert!(matches!(
        image(&before).check_created_field_addition_after_off_history_source(
            "Table1",
            &mut budget(),
            &|| true
        ),
        Err(FieldAdditionAfterOffHistoryError::Inspect(
            InspectError::Cancelled
        ))
    ));
    assert!(matches!(
        image(&before).check_created_field_addition_after_off_history_source(
            "Table1",
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(FieldAdditionAfterOffHistoryError::Inspect(
            InspectError::Limit(_)
        ))
    ));
}

#[test]
fn tight_fitted_history_replays_under_the_unchanged_five_million_step_pool() {
    let mut before = repeated::source(NumberSource::IdOnly);
    for ordinal in 0..5 {
        before = repeated::append(&before, &format!("N{ordinal}{}", "x".repeat(60)));
    }
    let off = canonical_database_options::append_explicit_off_to_owned_page(&before);
    let full = raw::append_number(&off, &"Z".repeat(59));
    check_source(&full).unwrap();
}
