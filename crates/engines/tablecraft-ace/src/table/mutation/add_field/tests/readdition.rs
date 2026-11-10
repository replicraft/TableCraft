//! Independent DROP-to-ADD images retain historical logical presence and payloads.
use super::{budget, equal_image, image, PAGE, SCOPE};
use crate::{CurrentFieldAdditionError, InspectError, read_table};
use std::io::Cursor;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field::populated::{
    CASES, readdition::{self as raw, Target},
};

#[test]
fn current_readdition_matches_independent_images_and_never_resurrects_retired_values() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            let before = raw::source(case);
            let expected = raw::after(&before, target);
            let source = image(&before);
            let facts = match target {
                Target::Number => {
                    source.inspect_current_field_addition_source("Table1", &mut budget(), &|| false)
                }
                Target::ShortText => source.inspect_current_short_text_field_addition_source(
                    "Table1",
                    &mut budget(),
                    &|| false,
                ),
            }
            .unwrap_or_else(|error| panic!("{case:?}/{target:?} inspection: {error:?}"));
            let verify = |actual: &[u8], active: usize| {
                equal_image(actual, &expected);
                let active_at = 7 * PAGE + 45;
                assert_eq!(
                    active,
                    usize::from(u16::from_le_bytes(
                        expected[active_at..active_at + 2].try_into().unwrap(),
                    ))
                );
                assert_eq!(&actual[8 * PAGE..10 * PAGE], &before[8 * PAGE..10 * PAGE]);
                let observed = read_table(
                    &mut Cursor::new(actual),
                    SCOPE,
                    "Table1",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(observed.rows(), raw::expected(case));
            };
            match target {
                Target::Number => {
                    let candidate = source
                        .prepare_current_number_field_addition(
                            "Table1",
                            target.name(),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap_or_else(|error| panic!("{case:?}/{target:?} candidate: {error:?}"));
                    assert_eq!(candidate.native_number(), facts.native_number());
                    assert_eq!(candidate.column_id(), facts.column_id());
                    verify(candidate.after_bytes(), candidate.column_count());
                }
                Target::ShortText => {
                    let candidate = source
                        .prepare_current_short_text_field_addition(
                            "Table1",
                            target.name(),
                            255,
                            true,
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap_or_else(|error| panic!("{case:?}/{target:?} candidate: {error:?}"));
                    assert_eq!(candidate.native_number(), facts.native_number());
                    assert_eq!(candidate.column_id(), facts.column_id());
                    verify(candidate.after_bytes(), candidate.column_count());
                }
            };
        }
    }
}

#[test]
fn readdition_honors_cancellation_and_explicit_zero_work_without_replacement() {
    let before = raw::source(CASES[0]);
    assert!(matches!(
        image(&before).prepare_current_number_field_addition(
            "Table1",
            "NextNumber",
            &mut budget(),
            &|| true,
        ),
        Err(CurrentFieldAdditionError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        image(&before).prepare_current_short_text_field_addition(
            "Table1",
            "NextText",
            255,
            true,
            &mut tablecraft_core::limits::ResourceBudget::new(0, 0, 0),
            &|| false,
        ),
        Err(CurrentFieldAdditionError::Inspect(InspectError::Limit(_)))
    ));
}
