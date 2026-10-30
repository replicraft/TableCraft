//! Literal source geometry and adversarial comparison for the independent proof.
use super::*;
use crate::IndexCheckStatus;
use crate::RowInsert;
use crate::check_table;
use crate::check_table_constraints;
use crate::table::mutation::insert::audit;
use crate::table::mutation::insert::compose;
use crate::table::mutation::insert::finish;
use crate::table::mutation::insert::preflight;
use crate::table::mutation::insert::prepare;
use crate::table::mutation::variable_update::map_audit::ExpectedByte;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::insert_seed as model;

const PAGE: usize = 4096;
const TABLE: usize = 10;
const ROOT: usize = 13;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn inline(members: &[usize]) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for number in members {
        bytes[5 + number / 8] |= 1 << (number % 8);
    }
    bytes
}

fn install(bytes: &mut [u8], number: usize, page: &[u8]) {
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

fn literal_wire(key: i32, reference: u32) -> [u8; 9] {
    let mut wire = [0; 9];
    wire[0] = 127;
    wire[1..5].copy_from_slice(&((key as u32) ^ 0x80000000).to_be_bytes());
    wire[5..].copy_from_slice(&reference.to_be_bytes());
    wire
}

fn source(code: u8, eof: usize, root: usize) -> Vec<u8> {
    let mut bytes = model::source(code, true, false, None);
    bytes.resize(eof * PAGE, 0);
    support::put32(&mut bytes, TABLE * PAGE + 16, 451);
    support::put32(&mut bytes, TABLE * PAGE + 67, 700);
    support::put32(&mut bytes, TABLE * PAGE + 181, root as u32);

    let values = (-226..0).chain(1..=225).collect::<Vec<i32>>();
    let mut entries = Vec::new();
    for (number, subset) in [(12, &values[..255]), (15, &values[255..])] {
        let rows = subset
            .iter()
            .map(|value| {
                let mut row = vec![2, 0];
                row.extend(value.to_le_bytes());
                row.extend([6, 0, 6, 0, 1, 0, 1]);
                (0, row)
            })
            .collect::<Vec<_>>();
        let mut page = support::data_page(TABLE as u32, &rows);
        support::put16(&mut page, 2, (4082 - rows.len() * 15) as u16);
        install(&mut bytes, number, &page);
        for (slot, value) in subset.iter().enumerate() {
            entries.push(literal_wire(*value, ((number as u32) << 8) | slot as u32));
        }
    }

    let mut leaf = vec![0xa7; PAGE];
    leaf[..480].fill(0);
    leaf[..2].copy_from_slice(&[4, 1]);
    support::put16(&mut leaf, 2, 7);
    support::put32(&mut leaf, 4, TABLE as u32);
    support::put16(&mut leaf, 24, 1);
    let mut end = 0;
    for (position, wire) in entries.iter().enumerate() {
        let suffix = &wire[usize::from(position != 0)..];
        leaf[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        leaf[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, 3609);
    bytes[ROOT * PAGE..(ROOT + 1) * PAGE].fill(0);
    install(&mut bytes, root, &leaf);
    install(
        &mut bytes,
        11,
        &support::data_page(0, &[(0, inline(&[12, 15])), (0, inline(&[12, 15]))]),
    );
    install(
        &mut bytes,
        14,
        &support::data_page(0, &[(0, inline(&[root]))]),
    );

    if eof > 512 {
        let bitmap = eof - 1;
        let mut page = vec![0; PAGE];
        page[..4].copy_from_slice(&[5, 1, 0, 0]);
        for number in eof..32736 {
            page[4 + number / 8] |= 1 << (number % 8);
        }
        install(&mut bytes, bitmap, &page);
        let mut indirect = vec![0; 69];
        indirect[0] = 1;
        support::put32(&mut indirect, 1, bitmap as u32);
        install(&mut bytes, 1, &support::data_page(1, &[(0, indirect)]));
    }

    bytes
}

fn checked(bytes: &[u8], count: usize) {
    let structural = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(structural.rows(), count);
    assert_eq!(structural.indexes().len(), 2);

    assert!(
        structural
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );

    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), count);
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

fn request() -> Vec<RowInsert> {
    vec![RowInsert::new(vec![Value::Int32(226), Value::Null])]
}

#[test]
fn root_growth_proof_covers_represented_rebased_and_promoted_inventory() {
    for code in [5, 6] {
        for (eof, root, base, right, left, metadata) in [
            (16, 13, 0, 16, 17, None),
            (598, 95, 0, 598, 599, None),
            (599, 95, 0, 599, 601, Some(600)),
            (599, 95, 88, 599, 600, Some(601)),
        ] {
            let mut before = source(code, eof, root);
            if base != 0 {
                let mut owned = vec![0; 69];
                support::put32(&mut owned, 1, base);
                let relative = root - base as usize;
                owned[5 + relative / 8] |= 1 << (relative % 8);
                install(&mut before, 14, &support::data_page(0, &[(0, owned)]));
                assert_eq!(right - base as usize, 511);
                assert_eq!(left - base as usize, 512);
            }

            // The relocated source is checked before either builder or finalizer
            // can classify the last represented bit or allocate metadata.

            checked(&before, 451);
            let image = capture(&before);
            let requests = request();

            let checked = preflight(&image, "Items", &requests, &mut budget(), &|| false).unwrap();
            let staged = prepare(&image, &checked, &requests, &mut budget(), &|| false).unwrap();
            assert_eq!(
                staged.indexes[0].added.len(),
                2 + usize::from(metadata.is_some())
            );

            let candidate =
                finish(image, "Items", &requests, &staged, &mut budget(), &|| false).unwrap();
            let after = candidate.candidate_bytes();
            self::checked(after, 452);
            let last = metadata.map_or(left, |number| left.max(number));
            assert_eq!(after.len(), (last + 1) * PAGE);
            assert_eq!(&after[root * PAGE..root * PAGE + 2], &[3, 1]);
            assert_eq!(
                &after[root * PAGE + 493..(root + 1) * PAGE],
                &before[root * PAGE + 493..(root + 1) * PAGE]
            );

            for number in [right, left] {
                let page = &after[number * PAGE..(number + 1) * PAGE];
                assert_eq!(&page[..2], &[4, 1]);
                assert_eq!(u16::from_le_bytes(page[2..4].try_into().unwrap()), 1582);
                assert_eq!(u16::from_le_bytes(page[24..26].try_into().unwrap()), 0);
                assert!(page[2514..].iter().all(|byte| *byte == 0));
            }

            if let Some(number) = metadata {
                let mut expected = vec![0; PAGE];
                expected[..4].copy_from_slice(&[5, 1, 0, 0]);
                for member in [root, right, left] {
                    expected[4 + member / 8] |= 1 << (member % 8);
                }
                assert_eq!(&after[number * PAGE..(number + 1) * PAGE], expected);
                assert_eq!(expected[4 + number / 8] & (1 << (number % 8)), 0);
            }
        }
    }
}

#[test]
fn healthy_growth_rejects_correlated_request_root_and_fresh_residue_forgery() {
    let before = source(5, 16, ROOT);
    checked(&before, 451);
    let image = capture(&before);
    let requests = request();
    let checked = preflight(&image, "Items", &requests, &mut budget(), &|| false).unwrap();
    let mut staged = prepare(&image, &checked, &requests, &mut budget(), &|| false).unwrap();
    let healthy = compose::compose(&image, &staged, &mut budget(), &|| false).unwrap();
    audit::prove(
        &image,
        "Items",
        &requests,
        &staged,
        &healthy,
        &mut budget(),
        &|| false,
    )
    .unwrap();

    staged.indexes[0].added[0].bytes[3000] = 0x37;
    let forged = compose::compose(&image, &staged, &mut budget(), &|| false).unwrap();
    self::checked(&forged, 452);
    assert_eq!(
        audit::prove(
            &image,
            "Items",
            &requests,
            &staged,
            &forged,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    );
    staged.indexes[0].added[0].bytes[3000] = 0;

    staged.indexes[0].patches[0].after[3000] ^= 1;
    let forged = compose::compose(&image, &staged, &mut budget(), &|| false).unwrap();
    self::checked(&forged, 452);
    assert_eq!(
        audit::prove(
            &image,
            "Items",
            &requests,
            &staged,
            &forged,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    );
    staged.indexes[0].patches[0].after[3000] ^= 1;

    let changed = vec![RowInsert::new(vec![Value::Int32(227), Value::Null])];
    assert_eq!(
        audit::prove(
            &image,
            "Items",
            &changed,
            &staged,
            &healthy,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    );
}

#[test]
fn index_row_transition_cannot_retire_even_unchanged_certified_data_effects() {
    for eof in [598, 599] {
        let before = source(5, eof, 95);
        checked(&before, 451);
        let image = capture(&before);
        let requests = request();
        let structural = preflight(&image, "Items", &requests, &mut budget(), &|| false).unwrap();
        let index = structural.indexes()[0].index();
        let mut cursor = image.reader();
        let mut allowance = budget();
        let mut reader = Reader {
            source: &mut cursor,
            header: image.catalog().header(),
            budget: &mut allowance,
            cancelled: &|| false,
            reads: 0,
        };

        let (mut entries, _) = decode(&before[95 * PAGE..96 * PAGE], &mut reader).unwrap();
        entries.push(Wire::read(&literal_wire(226, (15 << 8) | 196)).unwrap());
        let offset = 15 * PAGE - 69;
        assert_eq!(before[offset], 0);
        let mut proof = ImageProof {
            original: image.as_bytes(),
            bytes: before.clone(),
            qualified_zero_columns: Vec::new(),
            global: audit::global::Replay::new(&image, 10),
            map_edits: vec![ExpectedByte {
                offset,
                before: before[offset],
                after: before[offset],
            }],
        };

        assert!(matches!(
            derive(&image, index, 10, &entries, &mut proof, &mut reader),
            Err(EditError::Preservation)
        ));
    }
}
