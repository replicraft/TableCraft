//! Whole-current replay, not acceptance of a filtered or normalized source.
use super::*;
use crate::{
    FieldAdditionError,
    creation::database_options::field_history::tests::{budget, image, sources},
    format::reader::row_range,
};
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::{Source as NumberSource, repeated},
    canonical_database_options,
};

const PAGE: usize = 4096;
const DATABASE_NULL_DATE_BYTE: usize = 12;
const DICTIONARY_FIRST_NAME_BYTE: usize = 12;

fn current(before: &[u8]) -> Vec<u8> {
    canonical_database_options::append_explicit_off_to_owned_page(before)
}

#[test]
fn five_sdk_after_sources_qualify_only_through_complete_native_source_and_replay() {
    for before in sources() {
        let after = current(&before);
        let actual = image(&after);
        actual
            .check_field_history_name_auto_correct_off_source(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(actual.as_bytes(), after);
        // Neither an absent policy nor the already-OFF nine-role image enters
        // either existing before-OFF source language as a side effect.
        assert!(matches!(
            image(&before)
                .check_field_history_name_auto_correct_off_source(&mut budget(), &|| false),
            Err(Error::Source)
        ));
        assert!(
            actual
                .check_created_field_addition_source("Table1", &mut budget(), &|| false)
                .is_err()
        );
        assert!(
            actual
                .check_disable_name_auto_correct_source(&mut budget(), &|| false)
                .is_err()
        );
    }
}

#[test]
fn highest_fitting_n12_history_replays_under_the_unchanged_native_pool() {
    let mut before = repeated::source(NumberSource::IdOnly);
    for number in 2..=12 {
        before = repeated::append(&before, &format!("N{number}"));
    }
    let after = current(&before);
    let actual = image(&after);
    actual
        .check_field_history_name_auto_correct_off_source(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(actual.as_bytes(), after);
}

#[test]
fn full_replay_rejects_bytes_erased_by_hypothesis_even_when_live_policy_is_off() {
    let after = current(&sources().remove(4));
    let catalog = &after[4 * PAGE..5 * PAGE];
    let catalog_last = row_count(catalog).unwrap() - 1;
    let (_, catalog_start, _) = row_range(catalog, catalog_last).unwrap();
    let property = &after[10 * PAGE..11 * PAGE];
    let property_last = row_count(property).unwrap() - 1;
    let (_, property_start, _) = row_range(property, property_last).unwrap();
    for (offset, replacement) in [
        (4 * PAGE + catalog_start + DATABASE_NULL_DATE_BYTE, 1),
        (
            10 * PAGE + property_start + DICTIONARY_FIRST_NAME_BYTE,
            b't',
        ),
    ] {
        // NO_CLONE: Each corruption must start from the unchanged independently authored source.
        let mut changed = after.clone();
        assert_ne!(changed[offset], replacement);
        changed[offset] = replacement;
        let actual = image(&changed);
        let observed = inspect_database_properties(
            &mut actual.reader(),
            actual.catalog().catalog().scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            observed
                .name_auto_correct(&mut budget(), &|| false)
                .unwrap(),
            DatabaseNameAutoCorrect::ExplicitOff
        );
        let error = actual
            .check_field_history_name_auto_correct_off_source(&mut budget(), &|| false)
            .err();
        // The full replay rejects this producer-language source before mutation.
        assert!(
            matches!(error, Some(Error::Source)),
            "unexpected refusal at {offset}: {error:?}"
        );
        assert_eq!(actual.as_bytes(), changed);
    }
}

#[test]
fn hypothesis_does_not_admit_changed_original_tails_or_retired_history() {
    let after = current(&sources().remove(4));
    for offset in [3 * PAGE + 512, 7 * PAGE + 3000, 11 * PAGE - 1] {
        let mut changed = after.clone();
        changed[offset] ^= 1;
        let actual = image(&changed);
        assert!(matches!(
            actual.check_field_history_name_auto_correct_off_source(&mut budget(), &|| false),
            Err(Error::Source | Error::Preservation | Error::Field(FieldAdditionError::Source))
        ));
        assert_eq!(actual.as_bytes(), changed);
    }
}

#[test]
fn explicit_off_body_and_cancellation_resource_checks_are_not_skipped() {
    let after = current(&sources().remove(0));
    let property = &after[10 * PAGE..11 * PAGE];
    let (_, _, end) = row_range(property, row_count(property).unwrap() - 1).unwrap();
    let mut other = after.clone();
    assert_eq!(other[10 * PAGE + end - 1], 0);
    other[10 * PAGE + end - 1] = 1;
    assert!(matches!(
        image(&other).check_field_history_name_auto_correct_off_source(&mut budget(), &|| false),
        Err(Error::Source)
    ));
    let actual = image(&after);
    assert!(matches!(
        actual.check_field_history_name_auto_correct_off_source(&mut budget(), &|| true),
        Err(Error::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        actual.check_field_history_name_auto_correct_off_source(
            &mut ResourceBudget::new(0, 0, 0),
            &|| false,
        ),
        Err(Error::Inspect(InspectError::Limit(_)))
    ));
    assert_eq!(actual.as_bytes(), after);
}
