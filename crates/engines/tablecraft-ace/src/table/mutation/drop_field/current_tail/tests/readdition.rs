//! A repeated DROP retains original dictionaries and newly retired payloads.
use super::{budget, image, raw, SCOPE};
use crate::read_table;
use std::io::Cursor;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field::populated::readdition::{
    self, Target,
};

#[test]
fn current_tail_drop_repeats_after_readdition_and_materialization_without_losing_history() {
    for case in raw::CASES {
        for target in [Target::Number, Target::ShortText] {
            let added = readdition::after(&readdition::source(case), target);
            let filled = match target {
                Target::Number => readdition::fill_number(case, &added, 0, 123),
                Target::ShortText => readdition::fill_text(case, &added, 0, "fresh value"),
            };
            for before in [&added, &filled] {
                let expected = raw::after(before, target.name());
                let candidate = image(before)
                    .prepare_current_tail_field_drop(
                        "Table1",
                        target.name(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap_or_else(|error| panic!("{case:?}/{target:?}: {error:?}"));
                assert!(
                    candidate.after_bytes() == expected,
                    "{case:?}/{target:?}: first difference {:?}",
                    candidate
                        .after_bytes()
                        .iter()
                        .zip(&expected)
                        .position(|(actual, expected)| actual != expected)
                );
                let observed = read_table(
                    &mut Cursor::new(candidate.after_bytes()),
                    SCOPE,
                    "Table1",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(observed.rows(), raw::expected_rows(case));
            }
        }
    }
}
