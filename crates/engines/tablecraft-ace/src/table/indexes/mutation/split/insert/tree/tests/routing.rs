use super::*;
use crate::{
    IndexCheckStatus, TableCheck, check_table, check_table_constraints,
    test_support::{index_fixture, map_growth},
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::Resource;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

pub(super) fn source(code: u8, aliases: bool) -> Vec<u8> {
    let topology = index_fixture::node_database();
    let mut bytes = index_fixture::database(false, aliases, false);
    bytes.resize(19 * PAGE, 0);
    for page in [13, 14, 15, 16] {
        bytes[page * PAGE..(page + 1) * PAGE]
            .copy_from_slice(&topology[page * PAGE..(page + 1) * PAGE]);
    }
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    bytes[10 * PAGE + 189] = 0x89;
    bytes[10 * PAGE + 218] = 1;
    // This fresh wrapper gives the strong consumer complete declarations and
    // distinct writable map rows. The inherited fixture itself is unchanged.
    let inline = |members: &[usize]| {
        let mut row = vec![0; 69];
        for &page in members {
            row[5 + page / 8] |= 1 << (page % 8);
        }
        row
    };
    bytes[11 * PAGE..12 * PAGE].copy_from_slice(&support::data_page(
        0,
        &[(0, inline(&[12, 17, 18])), (0, inline(&[17, 18]))],
    ));
    support::put32(&mut bytes, 10 * PAGE + 59, (11 << 8) | 1);
    bytes[14 * PAGE..15 * PAGE]
        .copy_from_slice(&support::data_page(0, &[(0, inline(&[13, 15, 16]))]));
    for page in [17, 18] {
        let mut data = support::data_page(10, &[]);
        support::put16(&mut data, 2, 4082);
        bytes[page * PAGE..(page + 1) * PAGE].copy_from_slice(&data);
    }
    let future = (19..512).collect::<Vec<_>>();
    bytes[PAGE..2 * PAGE].copy_from_slice(&support::data_page(1, &[(0, inline(&future))]));
    for page in [2, 7, 10] {
        let end = map_growth::model::declaration_end(&bytes, page);
        support::put16(&mut bytes, end, 0xffff);
    }
    // Retained opaque tails make residue preservation observable.
    bytes[13 * PAGE + 600..14 * PAGE].fill(0xb3);
    bytes[15 * PAGE + 600..16 * PAGE].fill(0xc4);
    bytes[16 * PAGE + 600..17 * PAGE].fill(0xd5);
    bytes
}

pub(super) fn checked(bytes: &[u8]) -> TableCheck {
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

pub(super) fn capture(bytes: &[u8]) -> NativeImage {
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
            row: ((17 + position as u32 / 250) << 8) | (position as u32 % 250),
        });
    }
    result.sort_by_key(|entry| (entry.key, entry.row));
    result
}

pub(super) fn leaf(before: &[u8], entries: &[KeyRef]) -> Vec<u8> {
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

fn operation<'a>(
    image: &'a NativeImage,
    checked: &'a TableCheck,
    keys: &'a [KeyRef],
    profile: &'a SingleProfile,
) -> Operation<'a, SingleProfile> {
    Operation {
        image,
        checked,
        index: checked.indexes()[0].index(),
        profile,
        final_keys: keys,
    }
}

#[test]
fn original_bound_routes_left_right_both_and_noop_through_the_real_continuation() {
    let profile = SingleProfile {
        column: 1,
        kind: NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap(),
        descending: false,
    };
    for code in [5, 6] {
        for aliases in [false, true] {
            let before = source(code, aliases);
            let check = checked(&before);
            let image = capture(&before);
            for additions in [vec![5], vec![25], vec![40], vec![5, 25, 40], vec![]] {
                let final_keys = keys(&additions);
                let golden = expected(&before, &final_keys);
                let plan = continue_index::<_, ENTRY_BUFFER>(
                    operation(&image, &check, &final_keys, &profile),
                    State::new(&image, 10),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert!(plan.added.is_empty());
                assert_eq!(plan.append.length, before.len());
                let after = compose(&before, &plan.patches);
                assert_image(&after, &golden);
                if additions.is_empty() {
                    assert!(plan.patches.is_empty());
                }
            }
        }
    }
}

#[test]
fn source_seed_capacity_repair_remains_local_to_the_affected_leaf() {
    let profile = SingleProfile {
        column: 1,
        kind: NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap(),
        descending: false,
    };
    for code in [5, 6] {
        let before = source(code, true);
        let check = checked(&before);
        let image = capture(&before);
        let final_keys = keys(&(-400..0).collect::<Vec<_>>());
        let golden = expected(&before, &final_keys);
        assert_eq!(
            u16::from_le_bytes(golden[15 * PAGE + 24..15 * PAGE + 26].try_into().unwrap()),
            1
        );
        let plan = continue_index::<_, ENTRY_BUFFER>(
            operation(&image, &check, &final_keys, &profile),
            State::new(&image, 10),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(plan.added.is_empty());
        assert_image(&compose(&before, &plan.patches), &golden);
    }
}

#[test]
fn duplicate_bounds_changed_original_rows_and_malformed_source_topology_refuse() {
    let profile = SingleProfile {
        column: 1,
        kind: NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap(),
        descending: false,
    };
    let before = source(5, false);
    let check = checked(&before);
    let image = capture(&before);
    assert!(
        continue_index::<_, ENTRY_BUFFER>(
            operation(&image, &check, &keys(&[20]), &profile),
            State::new(&image, 10),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    let mut changed = keys(&[25]);
    changed[0].row = (17 << 8) | 199;
    assert_eq!(
        continue_index::<_, ENTRY_BUFFER>(
            operation(&image, &check, &changed, &profile),
            State::new(&image, 10),
            &mut budget(),
            &|| false
        )
        .err(),
        Some(EditError::Conflict)
    );
    for (offset, value) in [(13 * PAGE + 26, 2), (15 * PAGE + 16, 0)] {
        let mut invalid = before.clone();
        invalid[offset] = value;
        let image = capture(&invalid);
        assert_eq!(
            continue_index::<_, ENTRY_BUFFER>(
                operation(&image, &check, &keys(&[25]), &profile),
                State::new(&image, 10),
                &mut budget(),
                &|| false
            )
            .err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
    }
}

#[test]
fn migrated_kernel_quotas_cancel_and_retry_keep_original_bytes() {
    let profile = SingleProfile {
        column: 1,
        kind: NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap(),
        descending: false,
    };
    let before = source(6, true);
    let check = checked(&before);
    let image = capture(&before);
    let final_keys = keys(&[5, 25, 40]);
    for resource in [Resource::Bytes, Resource::Steps] {
        let mut limits = budget();
        limits.charge(resource, limits.remaining(resource)).unwrap();
        assert!(
            continue_index::<_, ENTRY_BUFFER>(
                operation(&image, &check, &final_keys, &profile),
                State::new(&image, 10),
                &mut limits,
                &|| false
            )
            .is_err()
        );
    }
    let calls = Cell::new(0);
    assert!(
        continue_index::<_, ENTRY_BUFFER>(
            operation(&image, &check, &final_keys, &profile),
            State::new(&image, 10),
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() > 20
            }
        )
        .is_err()
    );
    let plan = continue_index::<_, ENTRY_BUFFER>(
        operation(&image, &check, &final_keys, &profile),
        State::new(&image, 10),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_image(
        &compose(&before, &plan.patches),
        &expected(&before, &final_keys),
    );
    assert_eq!(image.as_bytes(), before);
}
