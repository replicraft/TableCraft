//! Opaque record bytes require independent preservation, not just equal values.
use super::super::{record as builder, roots};
use crate::{
    EditError, NativeImage, SlotScalarUpdate, check_table,
    table::mutation::gates,
    test_support::{base, support},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const DEFINITION_PAGE: usize = 10;
const DATA_PAGE: usize = 12;
const SCORE_FLAGS_OFFSET: usize = 103;
const SCORE_NUMBER_OFFSET: usize = 93;
const SCORE_VARIABLE_OFFSET: usize = 95;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 30_000_000)
}

fn database(dropped_variable: bool) -> Vec<u8> {
    let mut bytes = base::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    let mut definition = base::definition(&[("Id", 4, 0, 4), ("Score", 9, 0, 255)], 1, 11 << 8);
    definition[SCORE_FLAGS_OFFSET] = 2;
    support::put16(&mut definition, 43, 1);
    support::put32(&mut definition, 59, (11 << 8) | 1);
    let record = if dropped_variable {
        support::put16(&mut definition, 41, 3);
        support::put16(&mut definition, 43, 2);
        support::put16(&mut definition, SCORE_NUMBER_OFFSET, 2);
        support::put16(&mut definition, SCORE_VARIABLE_OFFSET, 1);
        // Logical column1/variable0 has no surviving schema field. Its two
        // opaque bytes precede Score, and its presence bit remains represented.
        vec![
            3, 0, 1, 0, 0, 0, 0xd1, 0xe2, 0x80, 0xa1, 0xb2, 0xc3, 9, 0, 8, 0, 6, 0, 2, 0, 7,
        ]
    } else {
        // Bytes6..8 are a physical fixed-region gap; bytes9..12 are allocated
        // padding after the one-byte Score. Neither has a visible column.
        vec![
            2, 0, 1, 0, 0, 0, 0xde, 0xad, 0x80, 0xa1, 0xb2, 0xc3, 9, 0, 8, 0, 1, 0, 3,
        ]
    };
    bytes[DEFINITION_PAGE * PAGE_BYTES..(DEFINITION_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&definition);
    let map = vec![0, 0, 0, 0, 0, 0, 0x10];
    bytes[11 * PAGE_BYTES..12 * PAGE_BYTES]
        .copy_from_slice(&support::data_page(0, &[(0, map.clone()), (0, map)]));
    let free = PAGE_BYTES - 16 - record.len();
    let mut page = support::data_page(10, &[(0, record)]);
    support::put16(&mut page, 2, free as u16);
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES].copy_from_slice(&page);
    let mut global = vec![0; 69];
    for number in 14..512 {
        global[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[PAGE_BYTES..2 * PAGE_BYTES].copy_from_slice(&support::data_page(1, &[(0, global)]));
    bytes
}

fn forged_record_audit(
    dropped_variable: bool,
    position: Option<usize>,
    bit: u8,
) -> Result<(), EditError> {
    let before = database(dropped_variable);
    let readable = check_table(
        &mut Cursor::new(&before),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let image = NativeImage::capture(
        &mut Cursor::new(&before),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let qualified = gates::before_slot(&image, "Items", &mut budget(), &|| false);
    let structural = if dropped_variable {
        // Historical dropped-column mutation stays refused. The ordinary
        // checked read schema isolates the private auditor's defensive span
        // contract without admitting this layout through the public service.
        assert_eq!(
            qualified.err(),
            Some(EditError::Unsupported(crate::WriteBlocker::RowLayout))
        );
        readable
    } else {
        qualified.unwrap()
    };
    let requests = [SlotScalarUpdate::new(
        0,
        Value::Binary(vec![0x80]),
        Value::Binary(vec![0x10, 0x20]),
    )];
    let captured = roots::capture(
        &image,
        structural.schema(),
        &requests,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let root = &captured.selected[0];
    let mut record = builder::build(
        structural.schema(),
        1,
        &root.final_record.data,
        root.update.value(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(record.len(), root.final_record.data.len());
    assert_eq!(&record[8..10], &[0x10, 0x20]);
    super::record(
        &image,
        structural.schema(),
        1,
        root,
        &record,
        &mut budget(),
        &|| false,
    )
    .unwrap();

    let position = position.unwrap_or(record.len() - 1);
    record[position] ^= bit;
    let mut forged = before;
    let offset = root.final_record.offset;
    forged[offset..offset + record.len()].copy_from_slice(&record);
    // Ordinary value/index checks cannot certify opaque record bytes. This
    // remains a readable final image with exactly the requested visible values.
    check_table(
        &mut Cursor::new(&forged),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    super::record(
        &image,
        structural.schema(),
        1,
        root,
        &record,
        &mut budget(),
        &|| false,
    )
}

#[test]
fn opaque_unused_and_deleted_mask_bits_are_preserved() {
    let actual = [
        forged_record_audit(false, None, 0x80),
        forged_record_audit(true, None, 0x02),
    ];
    assert_eq!(
        actual,
        [Err(EditError::Preservation), Err(EditError::Preservation)]
    );
}

#[test]
fn opaque_fixed_region_gap_is_preserved() {
    assert_eq!(
        forged_record_audit(false, Some(6), 0x40),
        Err(EditError::Preservation)
    );
}

#[test]
fn opaque_unrepresented_variable_payload_is_preserved() {
    assert_eq!(
        forged_record_audit(true, Some(6), 0x40),
        Err(EditError::Preservation)
    );
}

#[test]
fn unconsumed_original_tail_padding_is_preserved() {
    // Score grows into old padding byte9; old padding byte10 remains unused.
    assert_eq!(
        forged_record_audit(false, Some(10), 0x40),
        Err(EditError::Preservation)
    );
}
