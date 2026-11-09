//! Declaration-family controls are independent authored metadata/NULL records.
//! They issue neither a Source token nor mutation/admission permission.
use super::{qualify, qualify_selected, validate_selected};
use crate::{EditError, NativeImage, WriteBlocker, inspect_table_indexes, read_table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
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

fn ignored_manual_declaration(native: u8, width: u16) -> Vec<u8> {
    let mut bytes = witness(5, native, width, 0);
    let page = &bytes[DEFINITION..DEFINITION + PAGE];
    let physical = page
        .windows(4)
        .position(|word| word == [0x83, 0x07, 0, 0])
        .expect("literal independent physical index magic 1923");
    bytes[DEFINITION + physical + 46] |= 2;
    bytes
}

#[test]
fn selected_manual_i32_omission_is_a_declaration_only_not_other_family_authority() {
    let bytes = ignored_manual_declaration(4, 4);
    let original = image(&bytes);
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
        for family in [
            super::KeyFamily::GlobalInt32,
            super::KeyFamily::SparseIntegral,
            super::KeyFamily::SelectedScalar,
        ] {
            assert_refused(super::qualify_for(
                inspection.schema(),
                index,
                family,
                &mut budget(),
                &|| false,
            ));
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
    // Empty authored DATA does not qualify Source, programs or a write plan.
    assert_eq!(original.as_bytes(), bytes);
}

#[test]
fn ignored_selected_declaration_retains_type_flags_width_and_full_alias_guards() {
    for (native, width) in INTEGRAL {
        if native == 4 {
            continue;
        }
        let bytes = ignored_manual_declaration(native, width);
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
    for control in 0..5 {
        let mut bytes = ignored_manual_declaration(4, 4);
        let physical = bytes[DEFINITION..DEFINITION + PAGE]
            .windows(4)
            .position(|word| word == [0x83, 0x07, 0, 0])
            .unwrap();
        match control {
            0 => bytes[FIRST_COLUMN + 15] = 7,
            1 => bytes[FIRST_COLUMN + 16] = 0x40,
            2 => put16(&mut bytes, FIRST_COLUMN + 23, 5),
            // A later primary alias must reject the owner even when the first
            // visited logical alias is an ordinary secondary.
            3 => bytes[DEFINITION + physical + 52 + 28 + 23] = 1,
            4 => bytes[DEFINITION + physical + 52 + 28 + 23] = 2,
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

#[test]
fn selected_scalar_guid_declarations_do_not_expand_other_source_families() {
    for header in [5, 6] {
        for rows in [0, 2] {
            let bytes = witness(header, 15, 16, rows);
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
                super::qualify_for(
                    inspection.schema(),
                    index,
                    super::KeyFamily::SelectedScalar,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                for family in [
                    super::KeyFamily::GlobalInt32,
                    super::KeyFamily::SelectedIntegral,
                    super::KeyFamily::SparseIntegral,
                ] {
                    assert_refused(super::qualify_for(
                        inspection.schema(),
                        index,
                        family,
                        &mut budget(),
                        &|| false,
                    ));
                }
            }
            super::validate_retained_updates(
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
    // These authored NULL/empty metadata witnesses prove only declaration
    // isolation. They do not qualify Source or any GUID mutation target.
}

#[test]
fn selected_scalar_guid_requires_exact_manual_width_flags_and_every_alias() {
    for control in 0..5 {
        let mut bytes = witness(5, 15, 16, 0);
        match control {
            0 => bytes[FIRST_COLUMN + 15] = 0x43,
            1 => bytes[FIRST_COLUMN + 16] = 0x40,
            2 => put16(&mut bytes, FIRST_COLUMN + 23, 15),
            3 => put16(&mut bytes, FIRST_COLUMN + 23, 17),
            4 => {
                let physical = bytes[DEFINITION..DEFINITION + PAGE]
                    .windows(4)
                    .position(|word| word == [0x83, 0x07, 0, 0])
                    .unwrap();
                // A later foreign alias cannot hide behind an ordinary first
                // alias that shares the same physical data number/root.
                bytes[DEFINITION + physical + 52 + 28 + 23] = 2;
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
        assert!(matches!(
            super::validate_retained_updates(
                &original,
                inspection.schema(),
                inspection.indexes().iter(),
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Unsupported(WriteBlocker::ActiveContent)),
        ));
    }
}

// Independent raw record, not produced from declaration.rs format constants.
const TWO_KEY_PHYSICAL: [u8; 52] = [
    0x83, 0x07, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0xff, 0xff, 0x00, 0xff, 0xff, 0x00,
    0xff, 0xff, 0x00, 0xff, 0xff, 0x00, 0xff, 0xff, 0x00, 0xff, 0xff, 0x00, 0xff, 0xff, 0x00, 0xff,
    0xff, 0x00, 0x10, 0x03, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

fn tuple_metadata() -> crate::IndexInspection {
    let mut bytes = witness(5, 15, 16, 0);
    let before = inspect_table_indexes(
        &mut Cursor::new(&bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let start = DEFINITION + before.schema().layout.index_start;
    bytes[start..start + 52].copy_from_slice(&TWO_KEY_PHYSICAL);
    // Metadata only: the underlying NULL leaf is not a tuple Source witness.
    inspect_table_indexes(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn tuple_physical_golden_closes_every_ordered_key_and_preserves_scope_guards() {
    let inspected = tuple_metadata();
    assert_eq!(inspected.indexes().len(), 2);
    for index in inspected.indexes() {
        assert_eq!(index.columns().len(), 2);
        assert_eq!(
            (index.columns()[0].number(), index.columns()[0].descending()),
            (0, false)
        );
        assert_eq!(
            (index.columns()[1].number(), index.columns()[1].descending()),
            (1, true)
        );
        super::tuple_physical_record(&TWO_KEY_PHYSICAL, index, &mut budget(), &|| false).unwrap();
        for family in [
            super::KeyFamily::GlobalInt32,
            super::KeyFamily::SelectedIntegral,
            super::KeyFamily::SparseIntegral,
        ] {
            assert_refused(super::qualify_for(
                inspected.schema(),
                index,
                family,
                &mut budget(),
                &|| false,
            ));
        }
    }
}

#[test]
fn tuple_physical_extra_unused_direction_reserved_and_control_bytes_refuse() {
    let inspected = tuple_metadata();
    let index = &inspected.indexes()[0];
    // Active second key number/direction; hidden third key; unused direction;
    // reserved suffix; raw owner/root/flags; exact record-length closure.
    for (offset, value) in [
        (7, 0),
        (9, 1),
        (10, 0),
        (12, 1),
        (42, 1),
        (34, 0x11),
        (38, 10),
        (46, 0x81),
    ] {
        let mut raw = TWO_KEY_PHYSICAL;
        raw[offset] = value;
        assert_refused(super::tuple_physical_record(
            &raw,
            index,
            &mut budget(),
            &|| false,
        ));
    }
    assert_refused(super::tuple_physical_record(
        &TWO_KEY_PHYSICAL[..51],
        index,
        &mut budget(),
        &|| false,
    ));
    assert!(matches!(
        super::tuple_physical_record(&TWO_KEY_PHYSICAL, index, &mut budget(), &|| true),
        Err(EditError::Inspect(crate::InspectError::Cancelled)),
    ));
    assert!(matches!(
        super::tuple_physical_record(
            &TWO_KEY_PHYSICAL,
            index,
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(EditError::Inspect(crate::InspectError::Limit(_))),
    ));
}
