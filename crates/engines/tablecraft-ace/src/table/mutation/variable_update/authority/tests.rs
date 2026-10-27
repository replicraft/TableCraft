//! Canonical requested wires differ from semantically equivalent native records.
use super::super::{finish, placement::Plan};
use super::Operation;
use crate::{
    EditError, NativeImage, SlotScalarUpdate, check_table, read_table,
    table::mutation::{gates, page_update::stage},
    test_support::{base, support},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const DEFINITION_PAGE: usize = 10;
const DATA_PAGE: usize = 12;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 30_000_000)
}

fn source(marker_empty: bool) -> Vec<u8> {
    let mut bytes = base::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    let mut definition = base::definition(&[("Id", 4, 0, 4), ("Score", 10, 0, 510)], 1, 11 << 8);
    definition[103] = 2;
    definition[104] = 1;
    support::put16(&mut definition, 43, 1);
    support::put32(&mut definition, 59, (11 << 8) | 1);
    bytes[DEFINITION_PAGE * PAGE_BYTES..(DEFINITION_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&definition);

    let membership = vec![0, 0, 0, 0, 0, 0, 0x10];
    bytes[11 * PAGE_BYTES..12 * PAGE_BYTES].copy_from_slice(&support::data_page(
        0,
        &[(0, membership.clone()), (0, membership)],
    ));
    let payload = if marker_empty {
        [0xff, 0xfe]
    } else {
        [0x41, 0]
    };
    let mut record = vec![2, 0, 1, 0, 0, 0];
    record.extend(payload);
    record.extend([8, 0, 6, 0, 1, 0, 3]);
    let mut page = support::data_page(10, &[(0, record)]);
    support::put16(&mut page, 2, 4065);
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES].copy_from_slice(&page);

    let mut global = vec![0; 69];
    for number in 14..512 {
        global[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[PAGE_BYTES..2 * PAGE_BYTES].copy_from_slice(&support::data_page(1, &[(0, global)]));
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
fn alternative_equal_value_text_wire_is_rejected_after_complete_reader_controls() {
    let before = source(false);
    let image = capture(&before);
    let structural = gates::before_slot(&image, "Items", &mut budget(), &|| false).unwrap();
    let requests = [SlotScalarUpdate::new(
        0,
        Value::Text("A".into()),
        Value::Text("AB".into()),
    )];
    let operation = Operation::new("Items", "Score", &requests);
    let plan = Plan::capture(&image, structural.schema(), &mut budget(), &|| false).unwrap();
    let mut staged = stage::build_with(
        &image,
        &structural,
        1,
        &requests,
        plan,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    finish::finish(
        capture(&before),
        &operation,
        &staged.plan,
        &[],
        1,
        &mut budget(),
        &|| false,
    )
    .unwrap();

    let target = staged.plan.events[0].target.clone();
    let page = staged
        .plan
        .pages
        .iter_mut()
        .find(|page| page.number as usize == target.start / PAGE_BYTES)
        .unwrap();
    let start = target.start % PAGE_BYTES + 6;
    assert_eq!(&page.after[start..start + 4], &[0x41, 0, 0x42, 0]);
    page.after[start..start + 4].copy_from_slice(&[0xff, 0xfe, 0x41, 0x42]);

    let mut forged = before.clone();
    for page in &staged.plan.pages {
        let start = page.number as usize * PAGE_BYTES;
        forged[start..start + PAGE_BYTES].copy_from_slice(&page.after);
    }
    check_table(
        &mut Cursor::new(&forged),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let rows = read_table(
        &mut Cursor::new(&forged),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        rows.rows(),
        &[vec![Value::Int32(1), Value::Text("AB".into())]]
    );
    assert_eq!(
        finish::finish(
            capture(&before),
            &operation,
            &staged.plan,
            &[],
            1,
            &mut budget(),
            &|| false,
        )
        .err(),
        Some(EditError::Preservation)
    );
}

#[test]
fn marker_empty_semantic_noop_preserves_original_record_without_serialization() {
    let before = source(true);
    let image = capture(&before);
    let structural = gates::before_slot(&image, "Items", &mut budget(), &|| false).unwrap();
    let requests = [SlotScalarUpdate::new(
        0,
        Value::Text(String::new()),
        Value::Text(String::new()),
    )];
    let operation = Operation::new("Items", "Score", &requests);
    let verified = operation
        .prove(&image, &structural, &mut budget(), &|| false)
        .unwrap();
    assert!(verified.records.is_empty());
    let candidate = image
        .preview_ace16_variable_updates("Items", "Score", &requests, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.candidate_bytes(), before);
    assert_eq!(candidate.changed_rows(), 0);
}
