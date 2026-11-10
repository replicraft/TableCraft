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
        assert!(
            candidate.before_bytes() == before,
            "{case:?}: BEFORE image differs"
        );
        assert_eq!(candidate.after_bytes().len(), expected.len());
        let difference = candidate
            .after_bytes()
            .iter()
            .zip(&expected)
            .position(|(actual, expected)| actual != expected);
        assert_eq!(difference, None, "{case:?}: first AFTER difference");
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

#[test]
fn retired_variable_tail_keeps_full_native_index_and_survivor_read_checks() {
    let [before, expected] = raw::pair(raw::Case::TextAbsent);
    let source = source::capture(image(&before), "Table1", "NewLabel", &mut budget(), &|| {
        false
    })
    .unwrap_or_else(|error| panic!("source: {error:?}"));
    crate::check_table(
        &mut Cursor::new(&expected),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap_or_else(|error| panic!("after table check: {error:?}"));
    verify::survivors(&source, &expected, &mut budget(), &|| false)
        .unwrap_or_else(|error| panic!("survivors: {error:?}"));
}

#[test]
fn exact_group_retirement_rejects_orphan_live_groups_and_corrupted_dictionary_indices() {
    let before = raw::source(raw::Case::TextAbsent);
    let mut source = source::capture(image(&before), "Table1", "NewLabel", &mut budget(), &|| {
        false
    })
    .unwrap();
    let body = std::mem::take(&mut source.evidence.body);
    let change = catalog_property::compose_retained_single_at_origin(
        source.proof.image(),
        &source.evidence.observed.schema,
        body,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let expected = raw::after(&before, "NewLabel");
    super::super::verify::candidate(
        source.proof.image(),
        &source.evidence,
        &expected,
        &change,
        &mut budget(),
        &|| false,
    )
    .unwrap();

    let orphan = empty::remove_nonprimary(&before, "NewLabel", empty::Variant::DeclarationOnly);
    assert!(
        super::super::verify::candidate(
            source.proof.image(),
            &source.evidence,
            &orphan,
            &change,
            &mut budget(),
            &|| false
        )
        .is_err()
    );

    let mut corrupted = raw::after(&before, "NewLabel");
    // Locate the independently authored live row, then invalidate the first
    // survivor property's dictionary index without changing any length frame.
    let slot = (change.property_reference() & 0xff) as usize;
    let word = 10 * 4096 + 14 + 2 * slot;
    let start =
        usize::from(u16::from_le_bytes(corrupted[word..word + 2].try_into().unwrap()) & 0x1fff);
    let body = 10 * 4096 + start;
    let group =
        body + 4 + u32::from_le_bytes(corrupted[body + 4..body + 8].try_into().unwrap()) as usize;
    let name_bytes = usize::from(u16::from_le_bytes(
        corrupted[group + 10..group + 12].try_into().unwrap(),
    ));
    let dictionary_index = group + 12 + name_bytes + 4;
    corrupted[dictionary_index..dictionary_index + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(
        super::super::verify::candidate(
            source.proof.image(),
            &source.evidence,
            &corrupted,
            &change,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn actual_non_null_lvextra_carrier_blocks_populated_tail_deletion() {
    let before = raw::with_table_extra(raw::Case::TextPresent);
    let captured = image(&before);
    let schema = crate::inspect_table_schema(
        &mut captured.reader(),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let payloads = captured
        .inspect_object_payloads(schema.object_id(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(payloads.extra(), crate::PayloadPresence::Present);
    assert!(
        captured
            .inspect_current_tail_field_drop_source("Table1", "NewLabel", &mut budget(), &|| false,)
            .is_err()
    );
    assert!(
        captured
            .prepare_current_tail_field_drop("Table1", "NewLabel", &mut budget(), &|| false,)
            .is_err()
    );
}
