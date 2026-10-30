//! Independent source and complete-frame goldens for one primary-leaf split.
use super::*;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::map_growth::model::declaration_end;
use tablecraft_testkit::access::mutation_fixture as base;

pub(super) const PAGE: usize = 4096;
pub(super) const TABLE: usize = 10;
pub(super) const ROOT: usize = 13;
pub(super) const HISTORY: u32 = 700;

fn record(id: i32) -> Vec<u8> {
    let mut bytes = vec![2, 0];
    bytes.extend(id.to_le_bytes());
    bytes.extend([6, 0, 6, 0, 1, 0, 1]);
    bytes
}

fn inline(members: impl IntoIterator<Item = usize>) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for page in members {
        assert!(page < 512);
        bytes[5 + page / 8] |= 1 << (page % 8);
    }
    bytes
}

fn install(bytes: &mut [u8], number: usize, page: &[u8]) {
    assert_eq!(page.len(), PAGE);
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

fn wire(entry: &KeyRef) -> Vec<u8> {
    let value = i32::try_from(entry.key.unwrap()).unwrap();
    [
        vec![127],
        ((value as u32) ^ 0x80000000).to_be_bytes().to_vec(),
        entry.row.to_be_bytes().to_vec(),
    ]
    .concat()
}

fn leaf(entries: &[KeyRef], prefix: usize, previous: u32, next: u32, retained: bool) -> Vec<u8> {
    let mut bytes = vec![if retained { 0xa7 } else { 0 }; PAGE];
    bytes[..480].fill(0);
    bytes[..2].copy_from_slice(&[4, 1]);
    support::put32(&mut bytes, 4, TABLE as u32);
    support::put32(&mut bytes, 12, previous);
    support::put32(&mut bytes, 16, next);
    support::put16(&mut bytes, 24, prefix as u16);
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        let encoded = wire(entry);
        let shared = if position == 0 { 0 } else { prefix };
        let next = end + encoded.len() - shared;
        bytes[480 + end..480 + next].copy_from_slice(&encoded[shared..]);
        bytes[27 + next / 8] |= 1 << (next % 8);
        end = next;
    }
    support::put16(&mut bytes, 2, (3616 - end) as u16);
    bytes
}

pub(super) fn source(
    code: u8,
    count: usize,
    aliases: bool,
    distant: bool,
) -> (Vec<u8>, Vec<KeyRef>) {
    assert!((450..=451).contains(&count));
    let mut bytes = base::with_rule("Caption", "primary insertion split");
    bytes.resize(if distant { 600 * PAGE } else { 16 * PAGE }, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    let mut definition = base::definition(
        &[("ID", 4, 0, 4), ("Score", 9, 0, 255)],
        count as u32,
        11 << 8,
    );
    definition[88 + 15] = 2;
    support::put16(&mut definition, 88 + 7, 0);
    support::put16(&mut definition, 43, 1);
    definition.copy_within(63..131, 75);
    definition[63..75].fill(0);
    support::put32(&mut definition, 67, HISTORY);
    support::put32(&mut definition, 47, if aliases { 2 } else { 1 });
    support::put32(&mut definition, 51, 1);
    support::put32(&mut definition, 59, (11 << 8) | 1);
    for key in 0..10 {
        support::put16(&mut definition, 147 + 3 * key, 0xffff);
    }
    support::put16(&mut definition, 147, 0);
    definition[149] = 1;
    support::put32(&mut definition, 177, 14 << 8);
    support::put32(&mut definition, 181, ROOT as u32);
    definition[189] = 0x89;
    definition[218] = 1;
    if aliases {
        support::put32(&mut definition, 227, 1);
    }
    let mut at = 195 + if aliases { 56 } else { 28 };
    let names = if aliases {
        vec!["PrimaryID", "PrimaryAlias"]
    } else {
        vec!["PrimaryID"]
    };
    for name in names {
        let encoded = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        support::put16(&mut definition, at, encoded.len() as u16);
        at += 2;
        definition[at..at + encoded.len()].copy_from_slice(&encoded);
        at += encoded.len();
    }
    install(&mut bytes, TABLE, &definition);
    let values = [i32::MIN, i32::MAX]
        .into_iter()
        .chain((1..count - 1).map(|value| value as i32))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), count);
    let mut keys = Vec::new();
    for (page, subset) in [(12, &values[..255]), (15, &values[255..])] {
        let rows = subset.iter().map(|id| (0, record(*id))).collect::<Vec<_>>();
        let mut data = support::data_page(TABLE as u32, &rows);
        support::put16(&mut data, 2, (4082 - rows.len() * 15) as u16);
        install(&mut bytes, page, &data);
        for (slot, id) in subset.iter().enumerate() {
            keys.push(KeyRef {
                key: Some(i64::from(*id)),
                row: (page as u32) << 8 | slot as u32,
            });
        }
    }
    keys.sort_by_key(|entry| (entry.key, entry.row));
    install(&mut bytes, ROOT, &leaf(&keys, 1, 0, 0, true));
    install(
        &mut bytes,
        11,
        &support::data_page(0, &[(0, inline([12, 15])), (0, inline([12, 15]))]),
    );
    install(
        &mut bytes,
        14,
        &support::data_page(0, &[(0, inline([ROOT]))]),
    );
    let global = if distant {
        let mut map = vec![0; 69];
        map[0] = 1;
        map[1..5].copy_from_slice(&599_u32.to_le_bytes());
        let mut bitmap = vec![0; PAGE];
        bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
        for page in 600..32736 {
            bitmap[4 + page / 8] |= 1 << (page % 8);
        }
        install(&mut bytes, 599, &bitmap);
        map
    } else {
        inline(16..512)
    };
    install(&mut bytes, 1, &support::data_page(1, &[(0, global)]));
    for number in [2, 7, TABLE] {
        let end = declaration_end(&bytes, number);
        support::put16(&mut bytes, end, 0xffff);
    }
    (bytes, keys)
}

pub(super) fn final_keys(original: &[KeyRef], reference: u32) -> Vec<KeyRef> {
    let mut keys = original.to_vec();
    keys.push(KeyRef {
        key: Some(0),
        row: reference,
    });
    keys.sort_by_key(|entry| (entry.key, entry.row));
    keys
}

fn insert_data(bytes: &mut [u8], reference: u32) {
    let page = (reference >> 8) as usize;
    let slot = (reference & 255) as usize;
    if slot == 0 {
        let mut data = support::data_page(TABLE as u32, &[(0, record(0))]);
        support::put16(&mut data, 2, 4067);
        install(bytes, page, &data);
    } else {
        let data = &mut bytes[page * PAGE..(page + 1) * PAGE];
        assert_eq!(
            u16::from_le_bytes(data[12..14].try_into().unwrap()) as usize,
            slot
        );
        let end = u16::from_le_bytes(
            data[14 + (slot - 1) * 2..16 + (slot - 1) * 2]
                .try_into()
                .unwrap(),
        ) as usize;
        let row = record(0);
        let start = end - row.len();
        data[start..end].copy_from_slice(&row);
        support::put16(data, 14 + 2 * slot, start as u16);
        support::put16(data, 12, (slot + 1) as u16);
        support::put16(data, 2, (start - 14 - (slot + 1) * 2) as u16);
    }
}

pub(super) fn expected(
    before: &[u8],
    keys: &[KeyRef],
    reference: u32,
    seeded: bool,
    distant: bool,
) -> Vec<u8> {
    let frontier = before.len() / PAGE + usize::from(seeded);
    let right = frontier;
    let bitmap = distant.then_some(right + 1);
    let left = right + if distant { 2 } else { 1 };
    let mut after = before.to_vec();
    after.resize((left + 1) * PAGE, 0);
    insert_data(&mut after, reference);
    let middle = keys.len() / 2;
    install(
        &mut after,
        left,
        &leaf(&keys[..middle], 0, 0, right as u32, false),
    );
    install(
        &mut after,
        right,
        &leaf(&keys[middle..], 0, left as u32, 0, false),
    );
    let mut root = before[ROOT * PAGE..(ROOT + 1) * PAGE].to_vec();
    root[0] = 3;
    root[26] = 1;
    support::put32(&mut root, 20, right as u32);
    support::put16(&mut root, 24, 0);
    root[27..480].fill(0);
    let mut separator = wire(&keys[middle - 1]);
    separator.extend((left as u32).to_be_bytes());
    root[480..493].copy_from_slice(&separator);
    root[27 + 13 / 8] |= 1 << (13 % 8);
    support::put16(&mut root, 2, 3603);
    install(&mut after, ROOT, &root);
    let row_start = 14 * PAGE + 4027;
    if let Some(bitmap) = bitmap {
        let mut map = vec![0; 69];
        map[0] = 1;
        map[1..5].copy_from_slice(&(bitmap as u32).to_le_bytes());
        after[row_start..row_start + 69].copy_from_slice(&map);
        let mut payload = vec![0; PAGE];
        payload[..4].copy_from_slice(&[5, 1, 0, 0]);
        for page in [ROOT, right, left] {
            payload[4 + page / 8] |= 1 << (page % 8);
        }
        install(&mut after, bitmap, &payload);
    } else {
        for page in [right, left] {
            after[row_start + 5 + page / 8] |= 1 << (page % 8);
        }
    }
    if seeded {
        for start in [11 * PAGE + 4027, 11 * PAGE + 3958] {
            after[start + 5 + 16 / 8] |= 1 << (16 % 8);
        }
    }
    for page in before.len() / PAGE..after.len() / PAGE {
        let offset = if distant {
            599 * PAGE + 4 + page / 8
        } else {
            PAGE + 4027 + 5 + page / 8
        };
        after[offset] &= !(1 << (page % 8));
    }
    support::put32(&mut after, TABLE * PAGE + 16, keys.len() as u32);
    support::put32(&mut after, TABLE * PAGE + 67, HISTORY + 1);
    after
}

pub(super) fn compose(before: &[u8], plan: &Plan, reference: u32) -> Vec<u8> {
    let mut after = before.to_vec();
    after.resize(plan.append.length, 0);
    insert_data(&mut after, reference);
    for patch in &plan.patches {
        assert_eq!(
            &before[patch.offset..patch.offset + patch.before.len()],
            patch.before
        );
        after[patch.offset..patch.offset + patch.after.len()].copy_from_slice(&patch.after);
    }
    for page in &plan.added {
        assert_eq!(page.bytes.len(), PAGE);
        after[page.number as usize * PAGE..(page.number as usize + 1) * PAGE]
            .copy_from_slice(&page.bytes);
    }
    for promotion in &plan.promotions {
        let end = promotion.offset + promotion.before.len();
        assert_eq!(&before[promotion.offset..end], promotion.before);
        assert_eq!(promotion.before.len(), promotion.after.len());
        after[promotion.offset..end].copy_from_slice(&promotion.after);
    }
    for edit in &plan.append.map_edits {
        assert_eq!(before[edit.offset], edit.before);
        after[edit.offset] = edit.after;
    }
    let count = u32::from_le_bytes(
        before[TABLE * PAGE + 16..TABLE * PAGE + 20]
            .try_into()
            .unwrap(),
    );
    support::put32(&mut after, TABLE * PAGE + 16, count + 1);
    support::put32(&mut after, TABLE * PAGE + 67, HISTORY + 1);
    after
}

pub(super) fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "whole-image lengths differ");
    if actual != expected {
        let offset = actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right)
            .unwrap();
        panic!(
            "whole-image mismatch at byte {offset} (page {}, offset {}): actual {:02x}, expected {:02x}",
            offset / PAGE,
            offset % PAGE,
            actual[offset],
            expected[offset],
        );
    }
}
