//! Forged after-images must fail independently of the placement builder.
use super::super::{finish, placement::Plan};
use crate::{
    EditError, NativeImage, SlotScalarUpdate,
    limits::MAX_IMAGE_BYTES,
    table::mutation::{gates, page_update::stage},
    test_support::{base, support},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const ROOT_PAGE: usize = 12;
const EOF_PAGE: usize = 14;
const OWNER_OFFSET: usize = 4;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 30_000_000)
}

fn database() -> Vec<u8> {
    let mut bytes = base::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    let mut definition = base::definition(&[("Id", 4, 0, 4), ("Score", 9, 0, 255)], 1, 11 << 8);
    definition[103] = 2;
    support::put16(&mut definition, 43, 1);
    support::put32(&mut definition, 59, (11 << 8) | 1);
    bytes[10 * PAGE_BYTES..11 * PAGE_BYTES].copy_from_slice(&definition);
    let owned = vec![0, 0, 0, 0, 0, 0, 0x10];
    bytes[11 * PAGE_BYTES..12 * PAGE_BYTES]
        .copy_from_slice(&support::data_page(0, &[(0, owned.clone()), (0, owned)]));
    let record = vec![2, 0, 1, 0, 0, 0, b'A', 7, 0, 6, 0, 1, 0, 3];
    let mut page = support::data_page(10, &[(0, record), (0x8000, vec![0xab; 4000])]);
    support::put16(&mut page, 2, 64);
    bytes[ROOT_PAGE * PAGE_BYTES..(ROOT_PAGE + 1) * PAGE_BYTES].copy_from_slice(&page);
    let mut global = vec![0; 69];
    for number in EOF_PAGE..512 {
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

fn with_plan(check: impl FnOnce(&NativeImage, &mut Plan)) {
    let before = database();
    let image = capture(&before);
    let structural = gates::before_slot(&image, "Items", &mut budget(), &|| false).unwrap();
    let plan = Plan::capture(&image, structural.schema(), &mut budget(), &|| false).unwrap();
    let updates = [SlotScalarUpdate::new(
        0,
        Value::Binary(vec![b'A']),
        Value::Binary(vec![0x5a; 255]),
    )];
    let mut staged = stage::build_with(
        &image,
        &structural,
        1,
        &updates,
        plan,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    staged
        .audit(&image, &structural, 1, &mut budget(), &|| false)
        .unwrap();
    check(&image, &mut staged.plan);
}

#[test]
fn after_audit_rejects_forged_eof_headers_padding_and_old_hidden_bodies() {
    // Each mutation starts from a valid independently audited staged plan.
    for (number, offset) in [(EOF_PAGE, OWNER_OFFSET), (EOF_PAGE, 20), (ROOT_PAGE, 100)] {
        with_plan(|_, plan| {
            let page = plan
                .pages
                .iter_mut()
                .find(|page| page.number as usize == number)
                .unwrap();
            page.after[offset] ^= 1;
            assert_eq!(
                super::pages(plan, &mut budget(), &|| false),
                Err(EditError::Preservation)
            );
        });
    }
}

#[test]
fn append_finalizer_rejects_unauthorized_map_bits_and_rechecks_fresh_eof_header() {
    for mutation in 0..4 {
        with_plan(|image, plan| {
            match mutation {
                0 => plan.append.original_length -= PAGE_BYTES,
                1 => plan.append.length = image.as_bytes().len() - PAGE_BYTES,
                2 => plan.append.length += 1,
                _ => plan.append.length = MAX_IMAGE_BYTES as usize + PAGE_BYTES,
            }
            assert_eq!(
                finish::finish(
                    capture(image.as_bytes()),
                    "Items",
                    plan,
                    &[],
                    1,
                    &mut budget(),
                    &|| false,
                )
                .err(),
                Some(EditError::Preservation)
            );
        });
    }
    with_plan(|image, plan| {
        let valid = finish::finish(
            capture(image.as_bytes()),
            "Items",
            plan,
            &[],
            1,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(valid.candidate_bytes().len(), 15 * PAGE_BYTES);
        let edit = plan.append.map_edits.first_mut().unwrap();
        // Bit0 is not among the original page12 or appended page14 intents.
        edit.after ^= 1;
        assert_eq!(
            finish::finish(
                capture(image.as_bytes()),
                "Items",
                plan,
                &[],
                1,
                &mut budget(),
                &|| false
            )
            .err(),
            Some(EditError::Preservation)
        );
    });
    with_plan(|image, plan| {
        let page = plan
            .pages
            .iter_mut()
            .find(|page| page.number as usize == EOF_PAGE)
            .unwrap();
        page.after[OWNER_OFFSET] ^= 1;
        // Direct finalizer invocation bypasses the earlier staged-page audit:
        // reparsing actual grown bytes must still reject the wrong owner.
        assert!(
            finish::finish(
                capture(image.as_bytes()),
                "Items",
                plan,
                &[],
                1,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    });
}

#[test]
fn append_finalizer_rejects_hint_clear_without_an_actual_failed_placement() {
    with_plan(|image, plan| {
        let valid = finish::finish(
            capture(image.as_bytes()),
            "Items",
            plan,
            &[],
            1,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let rows = crate::read_table(
            &mut Cursor::new(valid.candidate_bytes()),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            rows.rows(),
            &[vec![Value::Int32(1), Value::Binary(vec![0x5a; 255])]]
        );
        let eof = &valid.candidate_bytes()[EOF_PAGE * PAGE_BYTES..(EOF_PAGE + 1) * PAGE_BYTES];
        assert_eq!(u16::from_le_bytes(eof[2..4].try_into().unwrap()), 3812);
        // The selected row was already placed successfully. A second record
        // of the same size would fit; no failed placement authorized this clear.
        plan.maps
            .failed_fit(
                image,
                EOF_PAGE as u32,
                &mut plan.append,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(
            finish::finish(
                capture(image.as_bytes()),
                "Items",
                plan,
                &[],
                1,
                &mut budget(),
                &|| false,
            )
            .err(),
            Some(EditError::Preservation)
        );
    });
}
