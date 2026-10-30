//! Request geometry must remain authoritative when staged ranges are forged.
use super::super::{authority::Operation, placement::Plan};
use crate::EditError;
use crate::NativeImage;
use crate::SlotScalarUpdate;
use crate::table::mutation::gates;
use crate::table::mutation::page_update::stage;
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as base;

const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 30_000_000)
}

fn source() -> Vec<u8> {
    let mut bytes = base::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;

    let mut definition = base::definition(&[("Id", 4, 0, 4), ("Score", 9, 0, 255)], 1, 11 << 8);
    definition[103] = 2;
    support::put16(&mut definition, 43, 1);
    support::put32(&mut definition, 59, (11 << 8) | 1);
    bytes[10 * PAGE..11 * PAGE].copy_from_slice(&definition);

    let membership = vec![0, 0, 0, 0, 0, 0, 0x10];
    bytes[11 * PAGE..12 * PAGE].copy_from_slice(&support::data_page(
        0,
        &[(0, membership.clone()), (0, membership)],
    ));

    let record = vec![2, 0, 1, 0, 0, 0, b'A', 7, 0, 6, 0, 1, 0, 3];
    let mut page = support::data_page(10, &[(0, record), (0x8000, vec![0xab; 4000])]);
    support::put16(&mut page, 2, 64);
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&page);

    let mut global = vec![0; 69];
    for number in 14..512 {
        global[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[PAGE..2 * PAGE].copy_from_slice(&support::data_page(1, &[(0, global)]));
    bytes
}

fn with_plan(
    value: Vec<u8>,
    inspect: impl FnOnce(&NativeImage, &mut Plan, &[super::ChangedRecord]),
) {
    let bytes = source();
    let image = NativeImage::capture(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let checked = gates::before_slot(&image, "Items", &mut budget(), &|| false).unwrap();
    let requests = [SlotScalarUpdate::new(
        0,
        Value::Binary(vec![b'A']),
        Value::Binary(value),
    )];
    let verified = Operation::new("Items", "Score", &requests)
        .prove(&image, &checked, &mut budget(), &|| false)
        .unwrap();
    let placement = Plan::capture(&image, checked.schema(), &mut budget(), &|| false).unwrap();
    let mut staged = stage::build_with(
        &image,
        &checked,
        1,
        &requests,
        placement,
        &mut budget(),
        &|| false,
    )
    .unwrap();

    super::prove(
        &image,
        checked.schema(),
        &staged.plan,
        &verified.records,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    inspect(&image, &mut staged.plan, &verified.records);
}

fn rejects(image: &NativeImage, plan: &Plan, records: &[super::ChangedRecord]) {
    let checked = gates::before_slot(image, "Items", &mut budget(), &|| false).unwrap();
    assert_eq!(
        super::prove(
            image,
            checked.schema(),
            plan,
            records,
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Preservation),
    );
}

#[test]
fn request_length_rejects_a_correlated_shorter_destination() {
    with_plan(vec![0x5a; 255], |image, plan, records| {
        assert_eq!(records[0].original_length, 14);
        assert_eq!(records[0].bytes.len(), 268);
        assert_eq!(plan.events[0].target, 15 * PAGE - 268..15 * PAGE);

        // A replay that derives length from this range still agrees with its
        // own invented start and map intents. The request independently needs
        // all 268 bytes, so that correlated geometry must be rejected.
        plan.events[0].target.start += 1;
        rejects(image, plan, records);
    });
}

#[test]
fn original_identity_and_capacity_determine_the_placement_branch() {
    for mutation in 0..4 {
        with_plan(vec![0x5a; 255], |image, plan, records| {
            match mutation {
                0 => plan.events[0].root ^= 1,
                1 => plan.events[0].word ^= 0x4000,
                2 => plan.events[0].root_range.start += 1,
                _ => plan.events[0].appended = None,
            }

            rejects(image, plan, records);
        });
    }

    with_plan(vec![b'B'], |image, plan, records| {
        assert_eq!(records[0].bytes.len(), 14);
        assert!(plan.events[0].appended.is_none());
        plan.events[0].appended = Some(0);
        rejects(image, plan, records);
    });

    with_plan(vec![b'A'], |image, plan, records| {
        assert!(records.is_empty());
        assert!(plan.events.is_empty());
        plan.append.owner ^= 1;
        rejects(image, plan, records);
    });
}
