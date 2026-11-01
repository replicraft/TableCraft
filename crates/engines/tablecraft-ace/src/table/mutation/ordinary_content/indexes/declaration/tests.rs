//! Declaration-family controls are independent authored metadata/NULL records.
//! They issue neither a Source token nor mutation/admission permission.
use super::{qualify, qualify_selected, validate_selected};
use crate::{EditError, NativeImage, WriteBlocker, inspect_table_indexes, read_table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::{
    canonical_ordinary_indexed::{self as sources, Case},
    data_page,
};

const PAGE: usize = 4096;
const DEFINITION: usize = 7 * PAGE;
const FIRST_COLUMN: usize = DEFINITION + 75;
const SECOND_COLUMN: usize = FIRST_COLUMN + 25;
const SCOPE: CatalogScope = CatalogScope::new(193);
const INTEGRAL: [(u8, u16); 5] = [(2, 1), (3, 2), (4, 4), (19, 8), (5, 8)];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn witness(header: u8, native: u8, width: u16, rows: usize) -> Vec<u8> {
    let source = sources::source(Case::AliasHistory);
    let mut bytes = source.bytes;
    bytes[20] = header;
    assert_eq!(bytes[FIRST_COLUMN], 4);
    assert_eq!(bytes[FIRST_COLUMN + 15], 3);
    assert_eq!(&bytes[FIRST_COLUMN + 23..FIRST_COLUMN + 25], &[4, 0]);
    assert_eq!(&bytes[SECOND_COLUMN + 21..SECOND_COLUMN + 23], &[4, 0]);
    bytes[FIRST_COLUMN] = native;
    put16(&mut bytes, FIRST_COLUMN + 23, width);
    // Keep nonoverlapping fixed declarations when the key's width changes.
    put16(&mut bytes, SECOND_COLUMN + 21, width);
    bytes[DEFINITION + 16..DEFINITION + 20].copy_from_slice(&(rows as u32).to_le_bytes());

    // Both logical cells are NULL. Payload bytes are intentionally inactive,
    // and cannot establish the index declaration's native carrier family.
    let mut record = vec![0xa5; 2 + usize::from(width) + 4 + 1];
    record[..2].copy_from_slice(&2_u16.to_le_bytes());
    *record.last_mut().unwrap() = 0;
    let records = (0..rows).map(|_| (0, record.clone())).collect::<Vec<_>>();
    bytes[8 * PAGE..9 * PAGE].copy_from_slice(&data_page(7, &records));

    // Literal ASC NULL key 00 followed by each original DATA8 row reference.
    // Empty and all-NULL trees therefore have identical key widths for every
    // native scalar type: an Int32 decoder cannot distinguish these families.
    let mut leaf = vec![0; PAGE];
    leaf[..2].copy_from_slice(&[4, 1]);
    leaf[4..8].copy_from_slice(&7_u32.to_le_bytes());
    let mut end = 0;
    for slot in 0..rows {
        leaf[480 + end] = 0;
        leaf[480 + end + 1..480 + end + 5]
            .copy_from_slice(&((8_u32 << 8) | slot as u32).to_be_bytes());
        end += 5;
        leaf[27 + end / 8] |= 1 << (end % 8);
    }
    put16(&mut leaf, 2, (3616 - end) as u16);
    bytes[9 * PAGE..10 * PAGE].copy_from_slice(&leaf);
    bytes
}

fn assert_refused(result: Result<(), EditError>) {
    assert!(matches!(
        result,
        Err(EditError::Unsupported(WriteBlocker::ActiveContent))
    ));
}

#[test]
fn empty_and_all_null_populations_do_not_expand_global_index_declarations() {
    for header in [5, 6] {
        for (native, width) in INTEGRAL {
            for rows in [0, 2] {
                let bytes = witness(header, native, width, rows);
                let original = image(&bytes);
                let snapshot = read_table(
                    &mut original.reader(),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(snapshot.rows().len(), rows);
                assert!(
                    snapshot.rows().iter().all(|row| row.len() == 2
                        && row.iter().all(|value| matches!(value, Value::Null)))
                );
                let inspection = inspect_table_indexes(
                    &mut original.reader(),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(inspection.indexes().len(), 2);
                for index in inspection.indexes() {
                    qualify_selected(inspection.schema(), index, &mut budget(), &|| false).unwrap();
                    let result = qualify(inspection.schema(), index, &mut budget(), &|| false);
                    if native == 4 {
                        result.unwrap();
                    } else {
                        assert_refused(result);
                    }
                }
                validate_selected(
                    &original,
                    inspection.schema(),
                    inspection.indexes().iter(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(original.as_bytes(), bytes);
            }
        }
    }
}

#[test]
fn selected_family_refuses_other_scalar_types_and_inexact_integral_shapes() {
    for (native, width) in [(1, 0), (6, 4), (7, 8), (8, 8), (15, 16)] {
        let bytes = witness(5, native, width, 0);
        let original = image(&bytes);
        let inspection = inspect_table_indexes(
            &mut original.reader(),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for index in inspection.indexes() {
            assert_refused(qualify_selected(
                inspection.schema(),
                index,
                &mut budget(),
                &|| false,
            ));
        }
    }
    for (native, width) in INTEGRAL {
        for control in [0, 1, 2] {
            let mut bytes = witness(5, native, width, 0);
            match control {
                0 => put16(&mut bytes, FIRST_COLUMN + 23, width + 1),
                1 => bytes[FIRST_COLUMN + 16] = 0x40,
                2 => {
                    bytes[FIRST_COLUMN] = 10;
                    bytes[FIRST_COLUMN + 15] = 2;
                    // One actual variable slot makes the metadata readable;
                    // no variable key is admitted by this integral family.
                    put16(&mut bytes, DEFINITION + 43, 1);
                }
                _ => unreachable!(),
            }
            let original = image(&bytes);
            let inspection = inspect_table_indexes(
                &mut original.reader(),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            for index in inspection.indexes() {
                assert_refused(qualify_selected(
                    inspection.schema(),
                    index,
                    &mut budget(),
                    &|| false,
                ));
            }
        }
    }
}

#[test]
fn selected_raw_validator_keeps_foreign_alias_and_ignore_null_refusals() {
    for case in [Case::IncompatibleAlias, Case::IgnoreNulls] {
        let source = sources::source(case);
        let original = image(&source.bytes);
        let inspection = inspect_table_indexes(
            &mut original.reader(),
            SCOPE,
            source.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            validate_selected(
                &original,
                inspection.schema(),
                inspection.indexes().iter(),
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Unsupported(WriteBlocker::ActiveContent))
        ));
    }
}
