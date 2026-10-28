//! Original requests, chunk ownership and inactive bytes remain proof authority.
use super::*;
use crate::table::mutation::insert::{
    long_values::Page,
    tests::long_value_inserts::{Family, external},
};

fn requested(length: usize, byte: u8) -> Vec<RowInsert> {
    vec![RowInsert::new(vec![
        Value::Int32(10),
        Value::Int32(100),
        Value::Binary(vec![byte; length]),
    ])]
}

fn prepared(length: usize) -> (NativeImage, Staged, Vec<RowInsert>, Vec<u8>) {
    let case = external::original(6, Family::Ole);
    let requests = requested(length, 0x9d);
    let expected = external::expected(&case, Family::Ole, &Value::Binary(vec![0x9d; length]));
    let (image, stage) = staged(&case.before, &requests);
    let after = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_eq!(after, expected);
    prove(
        &image,
        "Items",
        &requests,
        &stage,
        &after,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    (image, stage, requests, after)
}

fn refuses(image: &NativeImage, stage: &Staged, requests: &[RowInsert], after: &[u8]) {
    assert_eq!(
        prove(
            image,
            "Items",
            requests,
            stage,
            after,
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Preservation),
    );
}

#[test]
fn correlated_external_payloads_cannot_change_the_original_request() {
    let case = external::original(6, Family::Ole);
    for length in [65, 4077] {
        let original_requests = requested(length, 0x9d);
        let changed_requests = requested(length, 0x3a);
        let expected = external::expected(&case, Family::Ole, &Value::Binary(vec![0x3a; length]));
        let (image, stage) = staged(&case.before, &changed_requests);
        let changed = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
        assert_eq!(changed, expected);
        prove(
            &image,
            "Items",
            &changed_requests,
            &stage,
            &changed,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        refuses(&image, &stage, &original_requests, &changed);
    }
}

#[test]
fn correlated_chunk_bytes_links_and_unused_prefixes_are_rejected() {
    for (page_number, offset) in [(16, 20), (16, 24), (18, 4091)] {
        let (image, mut stage, requests, healthy) = prepared(4077);
        let page = stage
            .long_values
            .pages
            .iter_mut()
            .find(|page| page.number == page_number)
            .unwrap();
        // Page16 holds a4076-byte linked row at20; page18 holds the last
        // linked row at4087. Cover first link, payload and final payload.
        page.after[offset] ^= 1;
        let changed = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
        assert_ne!(changed, healthy);
        refuses(&image, &stage, &requests, &changed);
    }

    let (image, mut stage, requests, healthy) = prepared(65);
    // An unused byte outside the new single-row body still belongs to SOURCE.
    stage.long_values.pages[0].after[23] ^= 1;
    let changed = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_ne!(changed, healthy);
    checked(&changed, 4);
    refuses(&image, &stage, &requests, &changed);
}

#[test]
fn lval_column_page_inventory_and_descriptor_reserved_bytes_are_request_bound() {
    let (image, mut stage, requests, healthy) = prepared(65);
    stage.long_values.pages[0].column = 1;
    let changed = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_eq!(changed, healthy);
    refuses(&image, &stage, &requests, &changed);

    let (image, mut stage, requests, healthy) = prepared(65);
    let page = &stage.long_values.pages[0];
    let duplicate = Page {
        number: page.number,
        column: page.column,
        owned: page.owned,
        candidate: page.candidate,
        before: page.before.clone(),
        after: page.after.clone(),
    };
    stage.long_values.pages.push(duplicate);
    refuses(&image, &stage, &requests, &healthy);

    let (image, mut stage, requests, healthy) = prepared(65);
    let inserted = &stage.data.rows[0];
    let number = inserted.range.start / PAGE;
    let offset = inserted.range.start % PAGE;
    let page = stage
        .data
        .pages
        .iter_mut()
        .find(|page| page.number as usize == number)
        .unwrap();
    // C3, two fixedI32s, then the12-byte descriptor: its thirdword is reserved.
    page.after[offset + 18] = 1;
    let changed = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_ne!(changed, healthy);
    refuses(&image, &stage, &requests, &changed);
}
