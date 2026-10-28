//! Causal after-audit checks, independent of correct staging output.
use super::{forward::Plan, record, roots, space::Space, stage};
use crate::{
    EditError, NativeImage, SlotScalarUpdate, WriteBlocker, check_table, table::mutation::gates,
    test_support::base,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 30_000_000)
}

fn database() -> Vec<u8> {
    let mut bytes = base::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    let mut definition = base::definition(&[("Id", 4, 0, 4), ("Score", 9, 0, 255)], 1, 11 << 8);
    support::put16(&mut definition, 43, 1);
    support::put32(&mut definition, 59, (11 << 8) | 1);
    definition[103] = 2;
    bytes[10 * 4096..11 * 4096].copy_from_slice(&definition);
    let map = vec![0, 0, 0, 0, 0, 0, 0x10];
    bytes[11 * 4096..12 * 4096]
        .copy_from_slice(&support::data_page(0, &[(0, map.clone()), (0, map)]));
    let record = vec![2, 0, 1, 0, 0, 0, 0x80, 7, 0, 6, 0, 1, 0, 3];
    let mut page = support::data_page(10, &[(0, record)]);
    support::put16(&mut page, 2, 4066);
    bytes[12 * 4096..13 * 4096].copy_from_slice(&page);
    let mut global = vec![0; 69];
    for number in 14..512 {
        global[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[4096..8192].copy_from_slice(&support::data_page(1, &[(0, global)]));
    bytes
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

#[test]
fn forged_free_header_and_forward_pointer_fail_independent_after_page_audit() {
    let bytes = database();
    let image = capture(&bytes);
    let structural = gates::before_slot(&image, "Items", &mut budget(), &|| false).unwrap();
    let updates = [SlotScalarUpdate::new(
        0,
        Value::Binary(vec![0x80]),
        Value::Binary(vec![1; 7]),
    )];
    let mut staged =
        stage::build(&image, &structural, 1, &updates, &mut budget(), &|| false).unwrap();
    staged
        .audit(&image, &structural, 1, &mut budget(), &|| false)
        .unwrap();

    // Ordinary structural read checks do not certify the physical free count.
    staged.plan.pages[0].after[2] ^= 1;
    let mut forged = bytes.clone();
    forged[12 * 4096..13 * 4096].copy_from_slice(&staged.plan.pages[0].after);
    check_table(
        &mut Cursor::new(&forged),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        staged.audit(&image, &structural, 1, &mut budget(), &|| false),
        Err(EditError::Preservation)
    );
    staged.plan.pages[0].after[2] ^= 1;

    let root_start = usize::from(
        u16::from_le_bytes(bytes[12 * 4096 + 14..12 * 4096 + 16].try_into().unwrap()) & 0x1fff,
    );
    staged.plan.pages[0].after[root_start] ^= 1;
    assert_eq!(
        staged.audit(&image, &structural, 1, &mut budget(), &|| false),
        Err(EditError::Preservation)
    );

    let mut captured = roots::capture(
        &image,
        structural.schema(),
        &updates,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let root = &mut captured.selected[0];
    let replacement = record::build(
        structural.schema(),
        1,
        &root.final_record.data,
        root.update.value(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    root.range.end = root.range.start + 3;
    assert_eq!(
        Plan::new().apply(
            &image,
            root,
            &replacement,
            &staged.space,
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    );
}

#[test]
fn changed_future_free_bit_fails_protected_map_audit_even_when_table_checks_pass() {
    let bytes = database();
    let image = capture(&bytes);
    let structural = gates::before_slot(&image, "Items", &mut budget(), &|| false).unwrap();
    let space =
        Space::capture(&image, structural.schema(), &[12], &mut budget(), &|| false).unwrap();
    space
        .audit(&bytes, &bytes, &mut budget(), &|| false)
        .unwrap();
    let mut forged = bytes.clone();
    let map_start =
        usize::from(u16::from_le_bytes(bytes[4096 + 14..4096 + 16].try_into().unwrap()) & 0x1fff);
    forged[4096 + map_start + 5 + 100 / 8] ^= 1 << (100 % 8);
    check_table(
        &mut Cursor::new(&forged),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        space.audit(&bytes, &forged, &mut budget(), &|| false),
        Err(EditError::Preservation)
    );
}
