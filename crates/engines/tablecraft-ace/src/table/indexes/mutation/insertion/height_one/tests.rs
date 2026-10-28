use super::*;
use crate::{IndexCheckStatus, check_table, check_table_constraints, test_support::index_fixture};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::Resource;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn source(code: u8, aliases: bool) -> Vec<u8> {
    let topology = index_fixture::node_database();
    let mut bytes = index_fixture::database(false, aliases, false);
    bytes.resize(17 * PAGE, 0);
    for page in [13, 14, 15, 16] {
        bytes[page * PAGE..(page + 1) * PAGE]
            .copy_from_slice(&topology[page * PAGE..(page + 1) * PAGE]);
    }
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    bytes[10 * PAGE + 189] = 0x89;
    bytes[10 * PAGE + 218] = 1;
    // Retained opaque tails make residue preservation observable.
    bytes[13 * PAGE + 600..14 * PAGE].fill(0xb3);
    bytes[15 * PAGE + 600..16 * PAGE].fill(0xc4);
    bytes[16 * PAGE + 600..17 * PAGE].fill(0xd5);
    bytes
}

fn checked(bytes: &[u8]) -> TableCheck {
    let result = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(result.rows(), 3);
    assert!(
        result
            .indexes()
            .iter()
            .all(|entry| entry.status() == IndexCheckStatus::Checked)
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), 3);
    result
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

fn keys(additions: &[i32]) -> Vec<KeyRef> {
    let mut result = [10, 20, 30]
        .into_iter()
        .enumerate()
        .map(|(slot, value)| KeyRef {
            key: Some(value),
            row: (12 << 8) | slot as u32,
        })
        .collect::<Vec<_>>();
    for (position, value) in additions.iter().enumerate() {
        result.push(KeyRef {
            key: Some(i64::from(*value)),
            row: ((20 + position as u32 / 200) << 8) | (position as u32 % 200),
        });
    }
    result.sort_by_key(|entry| (entry.key, entry.row));
    result
}

fn leaf(before: &[u8], entries: &[KeyRef]) -> Vec<u8> {
    let wires = entries
        .iter()
        .map(|entry| {
            let value = i32::try_from(entry.key.unwrap()).unwrap();
            [
                vec![127],
                ((value as u32) ^ 0x80000000).to_be_bytes().to_vec(),
                entry.row.to_be_bytes().to_vec(),
            ]
            .concat()
        })
        .collect::<Vec<_>>();
    let mut common = 5;
    for wire in &wires[1..] {
        common = wires[0][..common]
            .iter()
            .zip(wire)
            .take_while(|(a, b)| a == b)
            .count();
    }
    let seed = usize::from(u16::from_le_bytes(before[24..26].try_into().unwrap()));
    let mut prefix = seed.min(common);
    if wires.len() * 9 - prefix * (wires.len() - 1) > 3616 {
        prefix = common;
    }
    let mut after = before.to_vec();
    after[27..480].fill(0);
    support::put16(&mut after, 24, prefix as u16);
    let mut end = 0;
    for (position, wire) in wires.iter().enumerate() {
        let suffix = &wire[if position == 0 { 0 } else { prefix }..];
        after[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        after[27 + end / 8] |= 1 << (end % 8);
    }
    assert!(end <= 3616);
    support::put16(&mut after, 2, (3616 - end) as u16);
    after
}

fn expected(before: &[u8], final_keys: &[KeyRef]) -> Vec<u8> {
    let split = final_keys.partition_point(|entry| entry.key <= Some(20));
    let mut after = before.to_vec();
    for (page, entries) in [(15, &final_keys[..split]), (16, &final_keys[split..])] {
        let start = page * PAGE;
        after[start..start + PAGE].copy_from_slice(&leaf(&before[start..start + PAGE], entries));
    }
    after
}

fn compose(before: &[u8], patches: &[IndexDataPatch]) -> Vec<u8> {
    let mut after = before.to_vec();
    for patch in patches {
        assert_eq!(
            &after[patch.offset..patch.offset + patch.before.len()],
            patch.before
        );
        assert!([15, 16].contains(&(patch.offset / PAGE)));
        after[patch.offset..patch.offset + patch.after.len()].copy_from_slice(&patch.after);
    }
    after
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    if let Some(offset) = actual.iter().zip(expected).position(|(a, b)| a != b) {
        panic!(
            "image differs at {offset}: actual {}, expected {}",
            actual[offset], expected[offset]
        );
    }
}

#[test]
fn original_bound_routes_left_right_and_both_without_changing_parent_or_maps() {
    for code in [5, 6] {
        for aliases in [false, true] {
            let before = source(code, aliases);
            let check = checked(&before);
            let image = capture(&before);
            for additions in [vec![5], vec![25], vec![40], vec![5, 25, 40], vec![]] {
                let final_keys = keys(&additions);
                let golden = expected(&before, &final_keys);
                let patches = prepare(&image, &check, 1, &final_keys, &mut budget(), &|| false)
                    .unwrap()
                    .unwrap();
                let after = compose(&before, &patches);
                assert_image(&after, &golden);
                assert_eq!(&after[13 * PAGE..15 * PAGE], &before[13 * PAGE..15 * PAGE]);
                if additions.iter().all(|value| *value > 20) {
                    assert_eq!(&after[15 * PAGE..16 * PAGE], &before[15 * PAGE..16 * PAGE]);
                }
                if additions.is_empty() {
                    assert!(patches.is_empty());
                }
            }
        }
    }
}

#[test]
fn capacity_repair_is_leaf_local_and_overflow_requests_continuation() {
    for code in [5, 6] {
        let before = source(code, true);
        let check = checked(&before);
        let image = capture(&before);
        let additions = (-400..0).collect::<Vec<_>>();
        let final_keys = keys(&additions);
        let golden = expected(&before, &final_keys);
        // More than 401 uncompressed nine-byte entries require prefix repair.
        assert_eq!(
            u16::from_le_bytes(golden[15 * PAGE + 24..15 * PAGE + 26].try_into().unwrap()),
            1
        );
        let patches = prepare(&image, &check, 1, &final_keys, &mut budget(), &|| false)
            .unwrap()
            .unwrap();
        assert_image(&compose(&before, &patches), &golden);
        assert!(
            prepare(
                &image,
                &check,
                1,
                &keys(&(-500..0).collect::<Vec<_>>()),
                &mut budget(),
                &|| false
            )
            .unwrap()
            .is_none()
        );
    }
}

#[test]
fn duplicate_boundary_changed_old_reference_and_unsupported_topologies_refuse() {
    let before = source(5, false);
    let check = checked(&before);
    let image = capture(&before);
    assert!(prepare(&image, &check, 1, &keys(&[20]), &mut budget(), &|| false).is_err());
    let mut changed = keys(&[25]);
    changed[0].row = 21 << 8;
    assert_eq!(
        prepare(&image, &check, 1, &changed, &mut budget(), &|| false).err(),
        Some(EditError::Preservation)
    );
    for (offset, value) in [(13 * PAGE + 26, 2), (13 * PAGE + 20, 0)] {
        let mut invalid = before.clone();
        invalid[offset] = value;
        assert_eq!(
            prepare(
                &capture(&invalid),
                &check,
                1,
                &keys(&[25]),
                &mut budget(),
                &|| false
            )
            .err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn)),
        );
    }
}

#[test]
fn quotas_late_cancellation_and_retry_do_not_change_original() {
    let before = source(6, true);
    let check = checked(&before);
    let image = capture(&before);
    let final_keys = keys(&[5, 25, 40]);
    for resource in [Resource::Bytes, Resource::Steps] {
        let mut limits = budget();
        limits.charge(resource, limits.remaining(resource)).unwrap();
        assert!(prepare(&image, &check, 1, &final_keys, &mut limits, &|| false).is_err());
    }
    let calls = Cell::new(0);
    assert!(
        prepare(&image, &check, 1, &final_keys, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            calls.get() > 20
        })
        .is_err()
    );
    assert_eq!(image.as_bytes(), before);
    let patches = prepare(&image, &check, 1, &final_keys, &mut budget(), &|| false)
        .unwrap()
        .unwrap();
    assert_image(&compose(&before, &patches), &expected(&before, &final_keys));
}
