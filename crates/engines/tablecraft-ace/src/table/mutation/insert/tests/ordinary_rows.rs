//! Full ordinary-row INSERT images with an indexed Int32 after another Int32.
use super::model as base;
use crate::{EditError, InspectError, RowInsert, WriteBlocker, inspect_table_schema, read_table};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

mod fixture;
mod model;
use fixture::{Case, Index, PAGE};
use model::Row;

fn checked(case: &Case, bytes: &[u8], rows: &[Row]) {
    base::checked(
        bytes,
        case.rows.len() + rows.len(),
        if matches!(case.index, Index::Alias) {
            2
        } else {
            1
        },
    );
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut base::budget(),
        &|| false,
    )
    .unwrap();
    let wanted = case
        .rows
        .iter()
        .chain(rows)
        .map(|row| row.values(case.variables))
        .collect::<Vec<_>>();
    assert_eq!(table.rows(), wanted);
}

fn run(case: &Case, rows: &[Row]) -> Vec<u8> {
    checked(case, &case.before, &[]);
    let expected = fixture::expected(case, rows);
    // Read every independently literal new record and complete tree before API.
    checked(case, &expected, rows);
    let requests = rows
        .iter()
        .map(|row| RowInsert::new(row.values(case.variables)))
        .collect::<Vec<_>>();
    let candidate = base::capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut base::budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.changed_rows(), rows.len());
    assert_eq!(candidate.candidate_bytes(), expected);
    checked(case, candidate.candidate_bytes(), rows);
    expected
}

fn refusal(case: &Case, values: Vec<Value>, expected: EditError) {
    checked(case, &case.before, &[]);
    let result = base::capture(&case.before).preview_ace16_insert_rows(
        "Items",
        &[RowInsert::new(values)],
        &mut base::budget(),
        &|| false,
    );
    assert_eq!(result.err(), Some(expected));
}

#[test]
fn seventeen_fields_bind_actual_primary_two_and_literal_fixed_bits_and_variable_ordinals() {
    for code in [5, 6] {
        for index in [Index::Primary, Index::Alias] {
            let case = fixture::source(code, true, false, index, false);
            assert_eq!(
                u16::from_le_bytes(
                    case.before[10 * PAGE + case.physical + 4..10 * PAGE + case.physical + 6]
                        .try_into()
                        .unwrap()
                ),
                2,
            );
            assert_eq!(case.before[10 * PAGE + case.logical + 23], 1);
            let mut row = Row::rich(10);
            row.earlier = None;
            row.last_flag = true;
            let record = row.record(true);
            assert_eq!(&record[..2], &[17, 0]);
            assert_eq!(&record[2..6], &[0; 4]);
            assert_eq!(record[7], 0);
            assert_eq!(&record[10..14], &10_i32.to_le_bytes());
            assert_eq!(&record[30..34], &0x7fc00123_u32.to_le_bytes());
            assert_eq!(&record[34..42], &0x8000000000000000_u64.to_le_bytes());
            assert_eq!(
                &record[50..66],
                &[
                    0x33, 0x22, 0x11, 0, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                    0xdd, 0xee, 0xff
                ]
            );
            // Payload order is BinaryFirst(var0), raw Score(var1), compressed LaterText(var2), BinaryLate(var3).
            assert_eq!(
                &record[70..82],
                &[
                    0, 0x80, 0xff, b'A', 0, b'B', 0, 0xff, 0xfe, b'A', b'B', b'C'
                ]
            );
            assert_eq!(&record[82..94], &[82, 0, 82, 0, 77, 0, 73, 0, 70, 0, 4, 0]);
            assert_eq!(&record[94..], &[0xfe, 0xff, 1]);
            run(&case, &[row]);
        }
    }
}

#[test]
fn fixed_only_has_no_variable_footer_and_nullable_spans_are_zero() {
    for code in [5, 6] {
        let case = fixture::source(code, false, false, Index::Alias, false);
        let mut row = Row::nulls(11);
        row.flag = true;
        row.last_flag = true;
        let record = row.record(false);
        assert_eq!(record.len(), 72);
        assert_eq!(&record[..2], &[13, 0]);
        assert_eq!(&record[10..14], &11_i32.to_le_bytes());
        assert!(record[2..10].iter().all(|byte| *byte == 0));
        assert!(record[14..70].iter().all(|byte| *byte == 0));
        assert_eq!(&record[70..], &[6, 0x10]);
        run(&case, &[row, Row::rich(12)]);
    }
}

#[test]
fn null_empty_and_preferred_text_controls_have_exact_boundaries_masks_and_decoding() {
    for code in [5, 6] {
        let case = fixture::source(code, true, false, Index::Primary, false);
        let nulls = Row::nulls(20);
        let mut empty = Row::nulls(21);
        empty.later_text = Some(String::new());
        empty.first_bytes = Some(Vec::new());
        empty.score = Some(String::new());
        empty.last_bytes = Some(Vec::new());
        let mut tie = Row::nulls(22);
        tie.later_text = Some("AB".into());
        tie.score = Some("ABC".into());
        let mut compressed = Row::nulls(23);
        compressed.later_text = Some("éÿx".into());
        let mut wide = Row::nulls(24);
        wide.later_text = Some("\0Ā\u{fffe}".into());
        let left = nulls.record(true);
        let right = empty.record(true);
        assert_eq!(left.len(), 85);
        assert_eq!(right.len(), 85);
        assert_eq!(&left[70..82], &[70, 0, 70, 0, 70, 0, 70, 0, 70, 0, 4, 0]);
        assert_eq!(&left[82..], &[4, 0, 0]);
        assert_eq!(&right[82..], &[4, 0x78, 0]);
        assert_eq!(
            &tie.record(true)[70..80],
            &[b'A', 0, b'B', 0, b'C', 0, b'A', 0, b'B', 0]
        );
        assert_eq!(
            &compressed.record(true)[70..75],
            &[0xff, 0xfe, 0xe9, 0xff, b'x']
        );
        assert_eq!(&wide.record(true)[70..76], &[0, 0, 0, 1, 0xfe, 0xff]);
        run(&case, &[nulls, empty, tie, compressed, wide]);
    }
}

#[test]
fn eof_multirow_preserves_old_deleted_capacity_and_extreme_payload_footers() {
    for code in [5, 6] {
        let case = fixture::source(code, true, true, Index::Alias, false);
        let mut row = Row::rich(30);
        row.first_bytes = Some(vec![0x81; 255]);
        row.last_bytes = Some(vec![0xfe; 510]);
        row.score = Some("Z".repeat(255));
        row.later_text = Some("😀".repeat(127) + "A");
        let record = row.record(true);
        assert_eq!(record.len(), 1870);
        assert_eq!(
            &record[1855..1867],
            &[0x3f, 7, 0x41, 5, 0x43, 3, 0x45, 1, 70, 0, 4, 0]
        );
        let expected = run(&case, &[row, Row::nulls(31)]);
        assert_eq!(expected.len(), 17 * PAGE);
        assert_eq!(
            &expected[12 * PAGE..13 * PAGE],
            &case.before[12 * PAGE..13 * PAGE]
        );
        assert_eq!(&expected[16 * PAGE..16 * PAGE + 2], &[1, 1]);
        assert_eq!(
            u32::from_le_bytes(expected[10 * PAGE + 67..10 * PAGE + 71].try_into().unwrap()),
            19
        );
    }
}

#[test]
fn exact_carriers_boolean_null_width_overflow_and_leading_marker_are_refused() {
    let case = fixture::source(6, true, false, Index::Primary, false);
    let good = Row::rich(40).values(true);
    for (position, replacement) in [
        (1, Value::Null),
        (16, Value::Null),
        (2, Value::Null),
        (3, Value::Int16(256)),
        (4, Value::Int32(32768)),
        (5, good[6].clone()),
        (6, good[5].clone()),
        (9, good[8].clone()),
        (10, Value::Binary(vec![0; 16])),
        (12, Value::Binary(vec![0; 256])),
        (14, Value::Binary(vec![0; 511])),
        (11, Value::Text("X".repeat(256))),
        (13, Value::Text("😀".repeat(128))),
    ] {
        let mut values = good.clone();
        values[position] = replacement;
        refusal(&case, values, EditError::Unsupported(WriteBlocker::Column));
    }
    for values in [
        Vec::new(),
        good[..16].to_vec(),
        [good.clone(), vec![Value::Null]].concat(),
    ] {
        refusal(&case, values, EditError::Unsupported(WriteBlocker::Column));
    }
    for position in [11, 13] {
        let mut values = good.clone();
        values[position] = Value::Text("\u{feff}".into());
        refusal(
            &case,
            values,
            EditError::Unsupported(WriteBlocker::RowLayout),
        );
    }
}

#[test]
fn readable_unqualified_index_and_layout_declarations_do_not_select_the_first_int32() {
    for index in [
        Index::Ordinary,
        Index::Descending,
        Index::Composite,
        Index::None,
    ] {
        let case = fixture::source(6, true, false, index, false);
        base::checked(
            &case.before,
            3,
            if matches!(index, Index::None) { 0 } else { 1 },
        );
        let result = base::capture(&case.before).preview_ace16_insert_rows(
            "Items",
            &[RowInsert::new(Row::rich(50).values(true))],
            &mut base::budget(),
            &|| false,
        );
        assert_eq!(
            result.err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
    }
    let mut case = fixture::source(6, true, false, Index::Primary, false);
    // A readable storage alias is not a disjoint writable fixed layout.
    case.before[10 * PAGE + 75 + 21..10 * PAGE + 75 + 23].copy_from_slice(&8_u16.to_le_bytes());
    base::checked(&case.before, 3, 1);
    let read = read_table(
        &mut Cursor::new(&case.before),
        CatalogScope::new(1),
        "Items",
        &mut base::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(read.rows()[0][0], Value::Int32(1));
    assert_eq!(read.rows()[0][2], Value::Int32(1));
    let result = base::capture(&case.before).preview_ace16_insert_rows(
        "Items",
        &[RowInsert::new(Row::rich(50).values(true))],
        &mut base::budget(),
        &|| false,
    );
    assert_eq!(
        result.err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );
    let mut case = fixture::source(6, true, false, Index::Primary, false);
    case.before[10 * PAGE + 75 + 11 * 25 + 7..10 * PAGE + 75 + 11 * 25 + 9]
        .copy_from_slice(&1_u16.to_le_bytes());
    let schema = inspect_table_schema(
        &mut Cursor::new(&case.before),
        CatalogScope::new(1),
        "Items",
        &mut base::budget(),
        &|| false,
    );
    assert!(matches!(schema, Err(InspectError::Malformed)));
    let result = base::capture(&case.before).preview_ace16_insert_rows(
        "Items",
        &[RowInsert::new(Row::rich(50).values(true))],
        &mut base::budget(),
        &|| false,
    );
    assert_eq!(
        result.err(),
        Some(EditError::Inspect(InspectError::Malformed))
    );
    let case = fixture::source(6, true, false, Index::DoublePrimary, false);
    let read = read_table(
        &mut Cursor::new(&case.before),
        CatalogScope::new(1),
        "Items",
        &mut base::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(read.rows().len(), 3);
    let result = base::capture(&case.before).preview_ace16_insert_rows(
        "Items",
        &[RowInsert::new(Row::rich(50).values(true))],
        &mut base::budget(),
        &|| false,
    );
    assert_eq!(
        result.err(),
        Some(EditError::Inspect(InspectError::Malformed))
    );
}

#[test]
fn defaults_generated_flags_pools_and_late_cancellation_keep_sources_retryable() {
    let case = fixture::source(6, true, false, Index::Primary, true);
    refusal(
        &case,
        Row::rich(60).values(true),
        EditError::Unsupported(WriteBlocker::Properties),
    );
    let mut generated = fixture::source(6, true, false, Index::Primary, false);
    generated.before[10 * PAGE + 75 + 2 * 25 + 15] = 0x13;
    refusal(
        &generated,
        Row::rich(60).values(true),
        EditError::Unsupported(WriteBlocker::Column),
    );
    let case = fixture::source(6, true, true, Index::Alias, false);
    let row = Row::rich(61);
    let expected = fixture::expected(&case, std::slice::from_ref(&row));
    checked(&case, &expected, std::slice::from_ref(&row));
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut budget = match resource {
            Resource::Bytes => ResourceBudget::new(0, 100_000, 100_000_000),
            Resource::Rows => ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
            Resource::Steps => ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
        };
        let result = base::capture(&case.before).preview_ace16_insert_rows(
            "Items",
            &[RowInsert::new(row.values(true))],
            &mut budget,
            &|| false,
        );
        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Limit(limit))) if limit.resource == resource)
        );
    }
    let calls = Cell::new(0_usize);
    let healthy = base::capture(&case.before)
        .preview_ace16_insert_rows(
            "Items",
            &[RowInsert::new(row.values(true))],
            &mut base::budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(healthy.candidate_bytes(), expected);
    let counted = base::capture(&case.before)
        .preview_ace16_insert_rows(
            "Items",
            &[RowInsert::new(row.values(true))],
            &mut base::budget(),
            &|| {
                calls.set(calls.get() + 1);
                false
            },
        )
        .unwrap();
    assert_eq!(counted.candidate_bytes(), expected);
    let threshold = calls.get() - 3;
    let count = Cell::new(0_usize);
    let result = base::capture(&case.before).preview_ace16_insert_rows(
        "Items",
        &[RowInsert::new(row.values(true))],
        &mut base::budget(),
        &|| {
            count.set(count.get() + 1);
            count.get() >= threshold
        },
    );
    assert_eq!(
        result.err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
}

#[test]
fn equal_length_semantically_identical_compressed_text_cannot_replace_preferred_raw() {
    use super::super::{compose, finish, preflight, prepare};
    let case = fixture::source(6, true, false, Index::Primary, false);
    let mut row = Row::nulls(70);
    row.later_text = Some("AB".into());
    let expected = run(&case, std::slice::from_ref(&row));
    let requests = [RowInsert::new(row.values(true))];
    let image = base::capture(&case.before);
    let before = preflight(&image, "Items", &requests, &mut base::budget(), &|| false).unwrap();
    let mut staged = prepare(&image, &before, &requests, &mut base::budget(), &|| false).unwrap();
    let offset = staged.data.rows[0].range.start;
    let page = staged
        .data
        .pages
        .iter_mut()
        .find(|page| page.number as usize == offset / PAGE)
        .unwrap();
    assert_eq!(
        &page.after[offset % PAGE + 70..offset % PAGE + 74],
        &[b'A', 0, b'B', 0]
    );
    page.after[offset % PAGE + 70..offset % PAGE + 74].copy_from_slice(&[0xff, 0xfe, b'A', b'B']);
    let forged = compose::compose(&image, &staged, &mut base::budget(), &|| false).unwrap();
    checked(&case, &forged, &[row]);
    assert_ne!(forged, expected);
    let result = finish(
        image,
        "Items",
        &requests,
        &staged,
        &mut base::budget(),
        &|| false,
    );
    assert_eq!(result.err(), Some(EditError::Preservation));
}

#[test]
fn allow_zero_length_is_an_operation_constraint_with_an_allowed_empty_control() {
    use crate::ConstraintViolation;
    use tablecraft_schema::FieldConstraintViolation;
    for code in [5, 6] {
        let mut denied = fixture::source(code, true, false, Index::Primary, false);
        fixture::empty_rules(&mut denied, &[("Score", false)]);
        let mut row = Row::nulls(80);
        row.score = Some(String::new());
        refusal(
            &denied,
            row.values(true),
            EditError::Inspect(InspectError::Constraint(ConstraintViolation::Field(
                FieldConstraintViolation::EmptyText,
            ))),
        );
        let mut allowed = fixture::source(code, true, false, Index::Primary, false);
        fixture::empty_rules(&mut allowed, &[("Score", true)]);
        run(&allowed, &[row]);
    }
}

#[test]
fn empty_text_requires_an_explicit_policy_while_nonempty_and_null_do_not() {
    let mut case = fixture::source(6, true, false, Index::Primary, false);
    fixture::remove_empty_rules(&mut case);
    run(&case, &[Row::rich(90), Row::nulls(91)]);
    for column in [11, 13] {
        let mut values = Row::nulls(92).values(true);
        values[column] = Value::Text(String::new());
        refusal(
            &case,
            values,
            EditError::Unsupported(WriteBlocker::ConstraintCoverage),
        );
    }
    let mut wrong = fixture::source(6, true, false, Index::Primary, false);
    fixture::wrongly_typed_empty_rule(&mut wrong);
    // Unknown policy typing is a before-state coverage failure, not a codec
    // rejection of this readable schema or a stored empty-value violation.
    let table = read_table(
        &mut Cursor::new(&wrong.before),
        CatalogScope::new(1),
        "Items",
        &mut base::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), 3);
    let result = base::capture(&wrong.before).preview_ace16_insert_rows(
        "Items",
        &[RowInsert::new(Row::rich(93).values(true))],
        &mut base::budget(),
        &|| false,
    );
    assert_eq!(
        result.err(),
        Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    );
}
