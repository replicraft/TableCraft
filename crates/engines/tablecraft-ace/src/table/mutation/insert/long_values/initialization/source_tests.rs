//! Complete source qualification is distinct from an empty-looking declaration.
use crate::{
    EditError, InspectError, LongValueCheckStatus, NativeImage, TableSchema, WriteBlocker,
    check_table,
    format::reader::Reader,
    inspect_table_schema,
    table::{
        long_value::{check_table_long_values_for_initialization, maps},
        mutation::{
            insert::tests::long_value_inserts::{Family, initialization::model},
            long_value::ownership::{Inventory, ZeroPair},
            variable_split::metadata::Metadata,
        },
    },
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const DEFINITION: usize = 10 * PAGE;
const DECLARATION: usize = DEFINITION + 498;
const POINTERS: usize = DECLARATION + 2;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn original(code: u8) -> model::Case {
    model::case(code, Family::Ole, Value::Binary(vec![0x92; 65]))
}

fn source(bytes: &[u8]) -> (NativeImage, TableSchema) {
    let image = NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    (image, schema)
}

fn qualify(image: &NativeImage, schema: &TableSchema) -> Result<Vec<ZeroPair>, EditError> {
    Inventory::capture_for_insert(image, schema, &[2], &[], &mut budget(), &|| false)
        .map(|(_, declarations)| declarations.zeros)
}

fn later_declaration(bytes: &mut [u8], column: u16, owned: u32, candidate: u32) {
    let at = DECLARATION + 10;
    bytes[at..at + 2].copy_from_slice(&column.to_le_bytes());
    bytes[at + 2..at + 6].copy_from_slice(&owned.to_le_bytes());
    bytes[at + 6..at + 10].copy_from_slice(&candidate.to_le_bytes());
    bytes[at + 10..at + 12].copy_from_slice(&65535_u16.to_le_bytes());
}

#[test]
fn zero_pair_qualification_visits_later_historical_maps_and_keeps_the_exact_td_span() {
    for code in [5, 6] {
        let mut case = original(code);
        // The original orphan page16 is now explicitly owned by a historical
        // declaration. Its distinct map rows remain protected source metadata.
        later_declaration(&mut case.before, 99, (11 << 8) | 2, (11 << 8) | 3);
        let (image, schema) = source(&case.before);
        let zeros = qualify(&image, &schema).unwrap();
        assert_eq!(zeros.len(), 1);
        assert_eq!(zeros[0].column, 2);
        assert_eq!(zeros[0].offset, POINTERS);
        let report = check_table_long_values_for_initialization(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            "Items",
            &[2],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(report.columns().len(), 2);
        assert_eq!(report.columns()[0].status(), LongValueCheckStatus::Absent);
        assert_eq!(report.columns()[1].column_number(), 99);
        assert_eq!(
            report.columns()[1].status(),
            LongValueCheckStatus::HistoricalDeclaration
        );
        assert_eq!(image.as_bytes(), case.before);
    }
}

#[test]
fn duplicate_truncated_and_half_zero_pairs_do_not_complete_the_source_grammar() {
    for mutation in 0..3 {
        let mut case = original(6);
        let expected = match mutation {
            0 => {
                later_declaration(&mut case.before, 2, 0, 0);
                EditError::Inspect(InspectError::Malformed)
            }
            1 => {
                // The true TD extent ends inside the selected ten-byte pair.
                case.before[DEFINITION + 8..DEFINITION + 12]
                    .copy_from_slice(&505_u32.to_le_bytes());
                EditError::Inspect(InspectError::Truncated)
            }
            2 => {
                case.before[POINTERS + 4..POINTERS + 8]
                    .copy_from_slice(&((11_u32 << 8) | 3).to_le_bytes());
                EditError::Unsupported(WriteBlocker::RowLayout)
            }
            _ => unreachable!(),
        };
        let (image, schema) = source(&case.before);
        assert_eq!(qualify(&image, &schema).err(), Some(expected));
        assert_eq!(image.as_bytes(), case.before);
    }
}

#[test]
fn a_complete_ffff_tail_is_absent_and_cannot_authorize_a_zero_pair_patch() {
    let mut case = original(6);
    case.before[DECLARATION..DECLARATION + 2].copy_from_slice(&65535_u16.to_le_bytes());
    let (image, schema) = source(&case.before);
    assert!(qualify(&image, &schema).unwrap().is_empty());
    let mut input = image.reader();
    let mut resources = budget();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget: &mut resources,
        cancelled: &|| false,
        reads: 0,
    };
    assert_eq!(
        maps::zero_pair_offset(&mut reader, &schema, 2),
        Err(InspectError::Unsupported)
    );
    assert_eq!(image.as_bytes(), case.before);
}

#[test]
fn existing_external_values_and_changed_row_geometry_do_not_prove_empty_source_ownership() {
    for mutation in 0..3 {
        let case = original(6);
        let mut before = if mutation == 0 {
            case.expected
        } else {
            case.before
        };
        let expected = match mutation {
            0 => {
                // The ordinary value reader can still follow the actual body19;
                // zero pointers do not erase that immutable external reference.
                before[POINTERS..POINTERS + 8].fill(0);
                EditError::Unsupported(WriteBlocker::RowLayout)
            }
            1 => {
                before[DEFINITION + 16..DEFINITION + 20].copy_from_slice(&4_u32.to_le_bytes());
                EditError::Inspect(InspectError::Malformed)
            }
            2 => {
                let data = 12 * PAGE;
                let end = usize::from(u16::from_le_bytes(
                    before[data + 16..data + 18].try_into().unwrap(),
                ));
                // The third record's first variable offset precedes fixed data.
                before[data + end - 5..data + end - 3].fill(0);
                EditError::Inspect(InspectError::Malformed)
            }
            _ => unreachable!(),
        };
        let (image, schema) = source(&before);
        assert_eq!(qualify(&image, &schema).err(), Some(expected));
        assert_eq!(image.as_bytes(), before);
    }
}

#[test]
fn zero_pair_exceptions_do_not_hide_aliases_of_mutable_ordinary_map_records() {
    for aliased in [false, true] {
        let mut case = original(6);
        let (owned, candidate) = if aliased {
            (11 << 8, (11 << 8) | 1)
        } else {
            ((11 << 8) | 2, (11 << 8) | 3)
        };
        later_declaration(&mut case.before, 99, owned, candidate);
        let (image, schema) = source(&case.before);
        let zeros = qualify(&image, &schema).unwrap();
        assert_eq!(zeros.len(), 1);
        let checked = check_table(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let result = Metadata::capture_for_insert_initialization(
            &image,
            &schema,
            checked.indexes(),
            &[2],
            &mut budget(),
            &|| false,
        );
        if aliased {
            assert_eq!(
                result.err(),
                Some(EditError::Unsupported(WriteBlocker::RowLayout))
            );
        } else {
            assert!(result.is_ok());
        }
        assert_eq!(image.as_bytes(), case.before);
    }
}
