//! Raw BEFORE/AFTER pairs are authored outside every native mutation producer.
use super::*;
use crate::{EditError, InspectError, read_table};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::Resource;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field::{
    self as empty, populated as raw,
};

const SCOPE: CatalogScope = CatalogScope::new(1729);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn inspection(error: &Error) -> Option<&InspectError> {
    match error {
        Error::Inspect(error) | Error::Placement(EditError::Inspect(error)) => Some(error),
        _ => None,
    }
}

#[test]
fn populated_tail_candidates_match_all_raw_pairs_and_exact_survivor_carriers() {
    for case in raw::CASES {
        let [before, expected] = raw::pair(case);
        let column = raw::column(case);
        let captured = image(&before);
        let inspected = captured
            .inspect_current_tail_field_drop_source("Table1", column, &mut budget(), &|| false)
            .unwrap();
        let candidate = captured
            .prepare_current_tail_field_drop("Table1", column, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(candidate.before_bytes(), before, "{case:?}");
        assert_eq!(candidate.after_bytes(), expected, "{case:?}");
        assert_eq!(candidate.table(), "Table1");
        assert_eq!(candidate.column(), column);
        assert_eq!(candidate.identity(), inspected);
        assert_eq!(
            candidate.identity().column_number(),
            usize::from(raw::identity(case).native_number)
        );
        assert_eq!(
            candidate.identity().column_id(),
            raw::identity(case).column_id
        );
        let rows = read_table(
            &mut Cursor::new(candidate.after_bytes()),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(rows.rows(), raw::expected_rows(case), "{case:?}");
        assert_eq!(candidate.declared_rows() as usize, rows.rows().len());
        for number in [0, 1, 2, 3, 5, 6, 8, 9] {
            assert_eq!(
                &candidate.after_bytes()[number * 4096..(number + 1) * 4096],
                &before[number * 4096..(number + 1) * 4096],
                "protected page {number} in {case:?}",
            );
        }
    }
}

#[test]
fn current_tail_and_original_empty_source_languages_remain_separate() {
    for case in raw::CASES {
        let before = raw::source(case);
        assert!(
            image(&before)
                .prepare_saved_field_drop("Table1", raw::column(case), &mut budget(), &|| false,)
                .is_err(),
            "legacy empty source must refuse {case:?}"
        );
        for column in ["ID", "missing"] {
            assert!(
                image(&before)
                    .prepare_current_tail_field_drop("Table1", column, &mut budget(), &|| false,)
                    .is_err()
            );
        }
    }
    let before = raw::source(raw::Case::NumberPresent);
    assert!(
        image(&before)
            .prepare_current_tail_field_drop("Table1", "Score", &mut budget(), &|| false,)
            .is_err(),
        "an unindexed middle field is not the native tail"
    );
    let before = empty::source(empty::SourceCase::NumberHistory);
    assert!(
        image(&before)
            .prepare_current_tail_field_drop("Table1", "Score2", &mut budget(), &|| false,)
            .is_err(),
        "an empty source cannot enter the populated family"
    );
}

#[test]
fn populated_tail_source_and_finish_preserve_budget_and_terminal_cancellation() {
    let before = raw::source(raw::Case::TextPresent);
    for (mut pool, resource) in [
        (
            ResourceBudget::new(1024, 100_000, 100_000_000),
            Resource::Bytes,
        ),
        (
            ResourceBudget::new(512 * 1024 * 1024, 0, 100_000_000),
            Resource::Rows,
        ),
        (
            ResourceBudget::new(512 * 1024 * 1024, 100_000, 0),
            Resource::Steps,
        ),
    ] {
        let error = image(&before)
            .prepare_current_tail_field_drop("Table1", "NewLabel", &mut pool, &|| false)
            .err()
            .expect("an exhausted explicit pool must refuse");
        assert!(
            matches!(inspection(&error), Some(InspectError::Limit(limit)) if limit.resource == resource)
        );
    }

    let calls = Cell::new(0_usize);
    let counted = || {
        calls.set(calls.get() + 1);
        false
    };
    image(&before)
        .prepare_current_tail_field_drop("Table1", "NewLabel", &mut budget(), &counted)
        .unwrap();
    let total = calls.get();
    for at in [1, total / 2, total] {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() >= at
        };
        let error = image(&before)
            .prepare_current_tail_field_drop("Table1", "NewLabel", &mut budget(), &cancelled)
            .err()
            .expect("sampled or terminal cancellation must refuse");
        assert!(matches!(inspection(&error), Some(InspectError::Cancelled)));
    }
}
