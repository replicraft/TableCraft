//! Independent native-read witnesses and execution of missing declarations.
mod execution;
pub(in crate::table::mutation::insert) mod model;
mod order;

use crate::{
    InspectError, LongValueCheckStatus, NativeImage, TableSchema, check_table_long_values,
    format::{
        allocation,
        bytes::u16_at,
        definition,
        reader::{Reader, live_row_range, row_count},
    },
    inspect_table_schema, read_table,
    table::{
        indexes,
        long_value::{maps, trace},
        mutation::insert::tests::long_value_inserts::{Family, initialization::model as zero},
        record_layout,
    },
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const DEFINITION: usize = 10 * PAGE;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn schema(image: &NativeImage) -> TableSchema {
    inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn readable(bytes: &[u8], expected: &[Vec<Value>]) {
    let snapshot = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(snapshot.rows(), expected);
    let image = capture(bytes);
    let metadata = schema(&image);
    assert_eq!(metadata.columns().len(), 3);
    assert_eq!(metadata.columns()[2].name(), "Score");
}

/// This prospective source certificate deliberately has no producer input.
/// Missing means a complete declaration grammar with no entry for column2.
fn missing_source(bytes: &[u8], external: bool) -> Result<usize, InspectError> {
    let image = capture(bytes);
    let schema = schema(&image);
    let mut input = image.reader();
    let mut resources = budget();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget: &mut resources,
        cancelled: &|| false,
        reads: 0,
    };
    let definitions = indexes::definitions_with_tail(&mut reader, &schema)?;
    let raw = definition::read(&mut reader, schema.definition_page())?;
    let declarations = maps::declarations(&mut reader, &schema)?;
    if !declarations.complete || declarations.entries.iter().any(|entry| entry.number == 2) {
        return Err(InspectError::Unsupported);
    }
    let terminal = definitions.tail + declarations.entries.len() * 10;
    if raw.len() > PAGE || raw.len() != terminal + 2 || raw[terminal..] != [255, 255] {
        return Err(InspectError::Unsupported);
    }
    let expected_free = PAGE.saturating_sub(raw.len() + 8);
    if usize::from(u16_at(&raw, 2)?) != expected_free {
        return Err(InspectError::Unsupported);
    }
    let new_length = raw
        .len()
        .checked_add(if external { 10 } else { 0 })
        .filter(|length| *length <= PAGE)
        .ok_or(InspectError::Unsupported)?;

    let field = &schema.columns()[2];
    let mut visited = 0;
    for page_number in allocation::pages(&mut reader, schema.layout.usage)? {
        let page = reader.page(page_number)?;
        for slot in 0..row_count(&page)? {
            if live_row_range(&page, slot)?.is_none() {
                continue;
            }
            let row =
                reader.row_location(page_number, slot as u8, schema.definition_page(), &page)?;
            visited += 1;
            let layout = record_layout::parse_values(&row.data, &schema, reader.budget)?;
            if field.number() >= layout.count
                || row.data[layout.mask_start + field.number() / 8] & (1 << (field.number() % 8))
                    == 0
            {
                continue;
            }
            let start = layout.offsets[field.field.variable];
            let end = layout.offsets[field.field.variable + 1];
            let traced = trace::read(&mut reader, &row.data[start..end])?;
            if !matches!(traced.kind, trace::Kind::Null | trace::Kind::Inline) {
                return Err(InspectError::Unsupported);
            }
        }
    }
    assert_eq!(visited, 3);
    Ok(new_length)
}

#[test]
fn honest_missing_sources_and_appended_expected_pairs_read_for_both_headers_and_families() {
    for code in [5, 6] {
        for family in [Family::Ole, Family::Memo] {
            for sibling in [false, true] {
                let case = model::external(code, family, sibling, false);
                readable(&case.before, &zero::rows(family, None));
                assert_eq!(missing_source(&case.before, true), Ok(case.new_length));
                assert_eq!(case.old_length, case.terminal + 2);
                assert_eq!(case.new_length, case.old_length + 10);
                readable(&case.expected, &zero::rows(family, Some(&case.value)));
                zero::checked_maps(&case.expected);
                assert_eq!(
                    &case.before[DEFINITION + case.terminal..DEFINITION + case.old_length],
                    &[255, 255],
                );
                assert_eq!(
                    &case.expected[DEFINITION + case.terminal..DEFINITION + case.terminal + 2],
                    &[2, 0],
                );
                if sibling {
                    assert_eq!(
                        &case.before[DEFINITION + 498..DEFINITION + 508],
                        &case.expected[DEFINITION + 498..DEFINITION + 508],
                    );
                }
            }
        }
    }
}

#[test]
fn inline_empty_insert_keeps_the_complete_missing_tail_and_source_maps_unchanged() {
    for code in [5, 6] {
        for family in [Family::Ole, Family::Memo] {
            let case = model::inline(code, family);
            readable(&case.before, &zero::rows(family, None));
            readable(&case.expected, &zero::rows(family, Some(&case.value)));
            assert_eq!(missing_source(&case.before, false), Ok(case.old_length));
            assert_eq!(case.new_length, case.old_length);
            assert_eq!(case.before.len(), case.expected.len());
            for range in [
                DEFINITION + 2..DEFINITION + 4,
                DEFINITION + 8..DEFINITION + 12,
                DEFINITION + 498..DEFINITION + PAGE,
                11 * PAGE..12 * PAGE,
            ] {
                assert_eq!(&case.before[range.clone()], &case.expected[range]);
            }
            let report = check_table_long_values(
                &mut Cursor::new(&case.expected),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(report.columns()[0].status(), LongValueCheckStatus::Absent);
            assert!(report.columns()[0].maps().is_none());
        }
    }
}

#[test]
fn malformed_truncated_trailer_and_wrong_free_are_not_missing_authority() {
    for code in [5, 6] {
        for mutation in 0..3 {
            let mut case = model::external(code, Family::Ole, true, false);
            match mutation {
                0 => case.before[DEFINITION + case.terminal + 1] = 0,
                1 => case.before[DEFINITION + 8..DEFINITION + 12]
                    .copy_from_slice(&((case.old_length - 1) as u32).to_le_bytes()),
                2 => case.before[DEFINITION + 2] ^= 1,
                _ => unreachable!(),
            }
            assert!(
                missing_source(&case.before, true).is_err(),
                "code {code}, mutation {mutation}"
            );
        }
    }
}

#[test]
fn an_absent_pair_does_not_erase_original_external_payload_ownership() {
    for code in [5, 6] {
        let case = model::external(code, Family::Ole, true, false);
        let mut before = case.expected;
        before[DEFINITION + case.terminal..DEFINITION + PAGE]
            .copy_from_slice(&case.before[DEFINITION + case.terminal..DEFINITION + PAGE]);
        before[DEFINITION + 2..DEFINITION + 4]
            .copy_from_slice(&case.before[DEFINITION + 2..DEFINITION + 4]);
        before[DEFINITION + 8..DEFINITION + 12]
            .copy_from_slice(&case.before[DEFINITION + 8..DEFINITION + 12]);
        readable(&before, &zero::rows(Family::Ole, Some(&case.value)));
        assert!(missing_source(&before, true).is_err());
    }
}

#[test]
fn actual_alias_definitions_reach_the_exact_single_page_extension_boundary() {
    for code in [5, 6] {
        let case = model::external(code, Family::Ole, false, true);
        assert_eq!((case.old_length, case.new_length), (4086, 4096));
        readable(&case.before, &zero::rows(Family::Ole, None));
        assert_eq!(missing_source(&case.before, true), Ok(4096));
        readable(&case.expected, &zero::rows(Family::Ole, Some(&case.value)));
        zero::checked_maps(&case.expected);
        let overflow = model::overflow_source(code, Family::Ole);
        readable(&overflow, &zero::rows(Family::Ole, None));
        assert!(missing_source(&overflow, true).is_err());
    }
}

#[test]
fn exact_nonzero_consumed_spare_is_replaceable_and_outside_bytes_stay_exact() {
    for code in [5, 6] {
        for family in [Family::Ole, Family::Memo] {
            let case = model::nonzero_spare(code, family);
            readable(&case.before, &zero::rows(family, None));
            assert_eq!(missing_source(&case.before, true), Ok(case.new_length));
            let consumed = &case.before[DEFINITION + case.old_length..DEFINITION + case.new_length];
            assert!(consumed.iter().all(|byte| *byte != 0));
            readable(&case.expected, &zero::rows(family, Some(&case.value)));
            zero::checked_maps(&case.expected);
            assert_eq!(
                &case.before[DEFINITION + case.new_length..DEFINITION + PAGE],
                &case.expected[DEFINITION + case.new_length..DEFINITION + PAGE],
            );
        }
    }
}
