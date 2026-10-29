//! Zero Memo maps compose with an existing Blob owner and primary root growth.
use super::{
    model::{self, Case, MemoMaps, Row},
    original,
};
use crate::{
    LongValueCheckStatus, RowInsert, check_table, check_table_long_values,
    format::{allocation, reader::Reader},
    read_table,
    table::{
        long_value::check_table_long_values_for_initialization,
        mutation::long_value::ownership::Inventory,
    },
};
use std::io::Cursor;
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;

pub(super) fn values(bytes: &[u8], rows: &[(Row, u32)]) {
    original::checked(bytes, rows.len(), 4);
    let snapshot = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let expected = rows.iter().map(|(row, _)| row.values()).collect::<Vec<_>>();
    assert_eq!(snapshot.rows(), expected);
}

fn row_range(bytes: &[u8], slot: usize) -> std::ops::Range<usize> {
    let page = 11 * PAGE;
    let start = usize::from(u16::from_le_bytes(
        bytes[page + 14 + slot * 2..page + 16 + slot * 2]
            .try_into()
            .unwrap(),
    ));
    let end = if slot == 0 {
        PAGE
    } else {
        usize::from(u16::from_le_bytes(
            bytes[page + 12 + slot * 2..page + 14 + slot * 2]
                .try_into()
                .unwrap(),
        ))
    };
    page + start..page + end
}

fn checked(case: &Case) {
    assert_eq!(case.before.len(), 20 * PAGE);
    assert_eq!(case.expected.len(), 26 * PAGE);
    assert_eq!(&case.before[10 * PAGE + 535..10 * PAGE + 543], &[0; 8]);
    assert_eq!(case.rows.len(), 400);
    assert_eq!(case.added.len(), 52);
    values(&case.before, &case.rows);
    let image = original::capture(&case.before);
    let table = check_table(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let (_, sources) = Inventory::capture_for_insert(
        &image,
        table.schema(),
        &[3],
        &[3, 2],
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(sources.zeros.len(), 1);
    assert_eq!(sources.zeros[0].column, 3);
    assert_eq!(sources.zeros[0].offset, 10 * PAGE + 535);
    assert!(sources.missing.is_empty());
    let source = check_table_long_values_for_initialization(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "Items",
        &[3],
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    for column in source.columns() {
        match column.column_number() {
            2 => assert_eq!(column.status(), LongValueCheckStatus::Checked),
            3 => assert_eq!(column.status(), LongValueCheckStatus::Absent),
            _ => panic!("two literal long declarations"),
        }
    }

    let final_rows = model::final_rows(case);
    assert_eq!(final_rows.len(), 452);
    assert!(
        final_rows[400..]
            .iter()
            .enumerate()
            .all(|(slot, (_, reference))| *reference == (23 << 8) | slot as u32)
    );
    values(&case.expected, &final_rows);
    let expected = check_table_long_values(
        &mut Cursor::new(&case.expected),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let mut input = Cursor::new(case.expected.as_slice());
    let expected_image = original::capture(&case.expected);
    let mut resources = original::budget();
    let mut reader = Reader {
        source: &mut input,
        header: expected_image.catalog().header(),
        budget: &mut resources,
        cancelled: &|| false,
        reads: 0,
    };
    for column in expected.columns() {
        assert_eq!(column.status(), LongValueCheckStatus::Checked);
        assert_eq!(column.shared_references(), 0);
        assert_eq!(column.shared_owned_pages(), 0);
        assert_eq!(column.values().single(), 52);
        let references = column.maps().unwrap();
        let owned = allocation::pages(&mut reader, references.owned()).unwrap();
        let hints = allocation::pages(&mut reader, references.candidate()).unwrap();
        match column.column_number() {
            2 => {
                assert_eq!(references.owned(), (11 << 8) | 4);
                assert_eq!(owned, [17, 22]);
                assert_eq!(hints, [22]);
                assert_eq!(column.orphan_rows(), 1);
            }
            3 => {
                assert_eq!(references.owned(), 20 << 8);
                assert_eq!(references.candidate(), (20 << 8) | 1);
                assert_eq!(owned, [21]);
                assert_eq!(hints, [21]);
                assert_eq!(column.orphan_rows(), 0);
            }
            _ => unreachable!(),
        }
    }
    assert_eq!(&case.expected[20 * PAGE..20 * PAGE + 4], &[1, 1, 100, 15]);
    assert_eq!(&case.expected[20 * PAGE + 4..20 * PAGE + 8], &[0; 4]);
    assert_eq!(&case.expected[20 * PAGE + 12..20 * PAGE + 14], &[2, 0]);
    assert_eq!(
        &case.expected[16 * PAGE..18 * PAGE],
        &case.before[16 * PAGE..18 * PAGE]
    );
    for slot in [2, 3] {
        let range = row_range(&case.before, slot);
        assert_eq!(&case.expected[range.clone()], &case.before[range]);
    }
    assert_eq!(
        u32::from_le_bytes(
            case.expected[10 * PAGE + 16..10 * PAGE + 20]
                .try_into()
                .unwrap()
        ),
        452
    );
    assert_eq!(
        u32::from_le_bytes(
            case.expected[10 * PAGE + 67..10 * PAGE + 71]
                .try_into()
                .unwrap()
        ),
        469
    );
    assert_eq!(
        u32::from_le_bytes(
            case.expected[10 * PAGE + 79..10 * PAGE + 83]
                .try_into()
                .unwrap()
        ),
        829
    );
}

#[test]
fn zero_memo_pressure_sources_and_expected_images_qualify_before_preview() {
    for code in [5, 6] {
        let case = model::source_with_memo_maps(code, true, MemoMaps::Zero);
        let mut existing = model::source_and_expected(code, true).before;
        existing[10 * PAGE + 535..10 * PAGE + 543].fill(0);
        assert_eq!(case.before, existing);
        checked(&case);
    }
}

#[test]
fn zero_memo_initialization_composes_with_both_lval_owners_data_and_primary_growth() {
    for code in [5, 6] {
        let case = model::source_with_memo_maps(code, true, MemoMaps::Zero);
        checked(&case);
        let requests = case
            .added
            .iter()
            .map(|row| RowInsert::new(row.values()))
            .collect::<Vec<_>>();
        let candidate = original::capture(&case.before)
            .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
            .unwrap();
        let actual = candidate.candidate_bytes();
        assert_eq!(actual.len(), case.expected.len());
        let difference = actual
            .iter()
            .zip(&case.expected)
            .position(|(actual, expected)| actual != expected);
        assert!(
            difference.is_none(),
            "zero-Memo composition difference {difference:?}"
        );
        values(actual, &model::final_rows(&case));
    }
}
