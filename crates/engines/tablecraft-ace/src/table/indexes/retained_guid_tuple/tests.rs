//! Metadata-only classification controls; these issue no Source or write token.
use super::*;
use crate::table::indexes::IndexColumn;
use crate::{IndexInspection, inspect_table_indexes};
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_ordinary_indexed::{self as fixture, Case};

const PAGE: usize = 4096;
const DEFINITION: usize = 7 * PAGE;
const FIRST_COLUMN: usize = DEFINITION + 75;
const SECOND_COLUMN: usize = FIRST_COLUMN + 25;
const SCOPE: CatalogScope = CatalogScope::new(304);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 10_000, 1_000_000)
}

fn metadata(native: u8, width: u16, flags: u8, extended: u8) -> IndexInspection {
    let mut raw = fixture::source(Case::AliasHistory).bytes;
    raw[FIRST_COLUMN] = native;
    raw[FIRST_COLUMN + 15] = flags;
    raw[FIRST_COLUMN + 16] = extended;
    raw[FIRST_COLUMN + 23..FIRST_COLUMN + 25].copy_from_slice(&width.to_le_bytes());
    raw[SECOND_COLUMN + 21..SECOND_COLUMN + 23].copy_from_slice(&width.to_le_bytes());
    let mut inspected = inspect_table_indexes(
        &mut Cursor::new(raw),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    // Private index metadata is changed only for this owning-module test.
    // No table payload, certificate, physical page or mutation is validated here.
    for index in &mut inspected.indexes {
        index.kind = 0;
        index.backing_primary = false;
        index.columns.clear();
        index.columns.push(IndexColumn {
            number: 0,
            descending: false,
        });
        index.columns.push(IndexColumn {
            number: 1,
            descending: true,
        });
    }
    inspected
}

#[test]
fn manual_tuple_metadata_accepts_both_orders_and_secondary_omission() {
    for ignored in [false, true] {
        for reversed in [false, true] {
            let mut inspected = metadata(15, 16, 3, 0);
            let index = &mut inspected.indexes[0];
            if ignored {
                index.flags |= 2;
            } else {
                index.flags &= !2;
            }
            if reversed {
                index.columns.reverse();
            }
            validate_manual_guid_tuple(
                &inspected.schema,
                &inspected.indexes[0],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        }
    }
}

#[test]
fn tuple_metadata_keeps_width_carrier_and_manual_field_guards() {
    for (native, width, flags, extended) in [
        (15, 15, 3, 0),
        (15, 17, 3, 0),
        (15, 16, 0x43, 0),
        (15, 16, 3, 0x40),
        (4, 4, 3, 0),
        (9, 16, 3, 0),
    ] {
        let inspected = metadata(native, width, flags, extended);
        assert!(matches!(
            validate_manual_guid_tuple(
                &inspected.schema,
                &inspected.indexes[0],
                &mut budget(),
                &|| false,
            ),
            Err(InspectError::Unsupported),
        ));
    }
}

#[test]
fn tuple_metadata_rejects_primary_omission_foreign_and_checks_controls() {
    let mut inspected = metadata(15, 16, 3, 0);
    inspected.indexes[0].kind = 1;
    inspected.indexes[0].flags |= 2;
    assert!(matches!(
        validate_manual_guid_tuple(
            &inspected.schema,
            &inspected.indexes[0],
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::Unsupported),
    ));
    inspected.indexes[0].kind = 2;
    assert!(matches!(
        validate_manual_guid_tuple(
            &inspected.schema,
            &inspected.indexes[0],
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::Unsupported),
    ));
    inspected.indexes[0].kind = 0;
    inspected.indexes[0].flags &= !2;
    assert!(
        validate_manual_guid_tuple(
            &inspected.schema,
            &inspected.indexes[0],
            &mut budget(),
            &|| true,
        )
        .is_err()
    );
    assert!(
        validate_manual_guid_tuple(
            &inspected.schema,
            &inspected.indexes[0],
            &mut ResourceBudget::new(0, 0, 0),
            &|| false,
        )
        .is_err()
    );
}
