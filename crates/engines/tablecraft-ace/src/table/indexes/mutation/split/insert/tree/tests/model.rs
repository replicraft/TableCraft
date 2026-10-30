//! Independent literal 451-entry child and complete INDEX-phase image.
//! DATA bodies and counters belong to the caller and remain untouched here.
use super::*;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::map_growth::model::declaration_end;
use tablecraft_testkit::access::mutation_fixture as base;

pub(super) const PAGE: usize = 4096;
const ROW_START: usize = 4027;

pub(super) struct Source {
    pub bytes: Vec<u8>,
    pub keys: Vec<KeyRef>,
    pub right: bool,
    pub distant: bool,
}

fn record(value: i32) -> Vec<u8> {
    let mut bytes = vec![2, 0];
    bytes.extend(value.to_le_bytes());
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
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

fn wire(key: &KeyRef) -> Vec<u8> {
    let value = i32::try_from(key.key.unwrap()).unwrap();
    [
        vec![127],
        ((value as u32) ^ 0x80000000).to_be_bytes().to_vec(),
        key.row.to_be_bytes().to_vec(),
    ]
    .concat()
}

fn frame(
    original: Option<&[u8]>,
    entries: &[(KeyRef, u32)],
    leaf: bool,
    links: (u32, u32),
    tail: u32,
) -> Vec<u8> {
    let mut bytes = original.map_or_else(|| vec![0; PAGE], <[u8]>::to_vec);
    let wires = entries
        .iter()
        .map(|(key, child)| {
            let mut bytes = wire(key);
            if !leaf {
                bytes.extend(child.to_be_bytes());
            }
            bytes
        })
        .collect::<Vec<_>>();
    let mut common = 9;
    for entry in &wires[1..] {
        common = wires[0][..common]
            .iter()
            .zip(entry)
            .take_while(|(a, b)| a == b)
            .count();
    }
    let seed = original.map_or(0, |before| {
        u16::from_le_bytes(before[24..26].try_into().unwrap()) as usize
    });
    let mut prefix = seed.min(common);
    let total = wires.iter().map(Vec::len).sum::<usize>();
    if total - prefix * (wires.len() - 1) > 3616 {
        prefix = common;
    }
    bytes[..27].fill(0);
    bytes[..2].copy_from_slice(&[if leaf { 4 } else { 3 }, 1]);
    support::put32(&mut bytes, 4, 10);
    support::put32(&mut bytes, 12, links.0);
    support::put32(&mut bytes, 16, links.1);
    support::put32(&mut bytes, 20, tail);
    support::put16(&mut bytes, 24, prefix as u16);
    bytes[26] = u8::from(!leaf);
    bytes[27..480].fill(0);
    let mut end = 0;
    for (position, wire) in wires.iter().enumerate() {
        let suffix = &wire[if position == 0 { 0 } else { prefix }..];
        bytes[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        bytes[27 + end / 8] |= 1 << (end % 8);
    }
    assert!(end <= 3616);
    support::put16(&mut bytes, 2, (3616 - end) as u16);
    bytes
}

fn leaf(keys: &[KeyRef], seed: usize, links: (u32, u32)) -> Vec<u8> {
    let mut before = vec![0xa7; PAGE];
    before[..480].fill(0);
    support::put16(&mut before, 24, seed as u16);
    frame(
        Some(&before),
        &keys.iter().copied().map(|key| (key, 0)).collect::<Vec<_>>(),
        true,
        links,
        0,
    )
}

pub(super) fn source(code: u8, aliases: bool, right: bool, distant: bool) -> Source {
    let mut bytes = base::with_rule("Caption", "primary child insertion split");
    bytes.resize(if distant { 600 * PAGE } else { 20 * PAGE }, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    let mut definition = base::definition(&[("ID", 4, 0, 4), ("Score", 9, 0, 255)], 452, 11 << 8);
    definition[88 + 15] = 2;
    support::put16(&mut definition, 88 + 7, 0);
    support::put16(&mut definition, 43, 1);
    definition.copy_within(63..131, 75);
    definition[63..75].fill(0);
    support::put32(&mut definition, 67, 700);
    support::put32(&mut definition, 47, if aliases { 2 } else { 1 });
    support::put32(&mut definition, 51, 1);
    support::put32(&mut definition, 59, (11 << 8) | 1);
    for position in 0..10 {
        support::put16(&mut definition, 147 + 3 * position, 0xffff);
    }
    support::put16(&mut definition, 147, 0);
    definition[149] = 1;
    support::put32(&mut definition, 177, 14 << 8);
    support::put32(&mut definition, 181, 13);
    definition[189] = 0x89;
    definition[218] = 1;
    if aliases {
        support::put32(&mut definition, 227, 1);
    }
    let mut at = 195 + if aliases { 56 } else { 28 };
    for name in if aliases {
        vec!["PrimaryID", "PrimaryAlias"]
    } else {
        vec!["PrimaryID"]
    } {
        let encoded = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        support::put16(&mut definition, at, encoded.len() as u16);
        at += 2;
        definition[at..at + encoded.len()].copy_from_slice(&encoded);
        at += encoded.len();
    }
    install(&mut bytes, 10, &definition);
    let values = if right {
        std::iter::once(-1)
            .chain(1..=450)
            .chain(std::iter::once(i32::MAX))
            .collect::<Vec<_>>()
    } else {
        std::iter::once(i32::MIN)
            .chain(-450..0)
            .chain(std::iter::once(1))
            .collect::<Vec<_>>()
    };
    assert_eq!(values.len(), 452);
    let mut keys = Vec::new();
    for (page, subset) in [(12, &values[..255]), (15, &values[255..])] {
        let rows = subset
            .iter()
            .map(|value| (0, record(*value)))
            .collect::<Vec<_>>();
        let mut data = support::data_page(10, &rows);
        support::put16(&mut data, 2, (4082 - 15 * rows.len()) as u16);
        install(&mut bytes, page, &data);
        keys.extend(subset.iter().enumerate().map(|(slot, value)| KeyRef {
            key: Some(i64::from(*value)),
            row: (page as u32) << 8 | slot as u32,
        }));
    }
    let boundary = if right { 1 } else { 451 };
    install(
        &mut bytes,
        17,
        &leaf(&keys[..boundary], if right { 0 } else { 1 }, (0, 18)),
    );
    install(
        &mut bytes,
        18,
        &leaf(&keys[boundary..], if right { 1 } else { 0 }, (17, 0)),
    );
    let mut root_seed = vec![0xb3; PAGE];
    root_seed[..480].fill(0);
    install(
        &mut bytes,
        13,
        &frame(
            Some(&root_seed),
            &[(keys[boundary - 1], 17)],
            false,
            (0, 0),
            18,
        ),
    );
    install(
        &mut bytes,
        11,
        &support::data_page(0, &[(0, inline([12, 15])), (0, inline([12, 15]))]),
    );
    install(
        &mut bytes,
        14,
        &support::data_page(0, &[(0, inline([13, 17, 18]))]),
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
        inline(20..512)
    };
    install(&mut bytes, 1, &support::data_page(1, &[(0, global)]));
    for number in [2, 7, 10] {
        let end = declaration_end(&bytes, number);
        support::put16(&mut bytes, end, 0xffff);
    }
    Source {
        bytes,
        keys,
        right,
        distant,
    }
}

pub(super) fn final_keys(source: &Source, seeded: bool, peer: bool) -> Vec<KeyRef> {
    let mut keys = source.keys.clone();
    keys.push(KeyRef {
        key: Some(if source.right { 451 } else { -451 }),
        row: if seeded {
            (source.bytes.len() as u32 / 4096) << 8
        } else {
            (15 << 8) | 197
        },
    });
    if peer {
        keys.push(KeyRef {
            key: Some(if source.right { -2 } else { 2 }),
            row: (15 << 8) | 198,
        });
    }
    keys.sort_by_key(|key| (key.key, key.row));
    keys
}

pub(super) fn expected(source: &Source, keys: &[KeyRef], seeded: bool) -> Vec<u8> {
    let before = &source.bytes;
    let fresh = before.len() / PAGE + usize::from(seeded);
    let bitmap = source.distant.then_some(fresh + 1);
    let mut after = before.clone();
    after.resize((fresh + 1 + usize::from(bitmap.is_some())) * PAGE, 0);
    let boundary = keys.partition_point(|key| key.key <= Some(-1));
    let partitions = [&keys[..boundary], &keys[boundary..]];
    let target = usize::from(source.right);
    let middle = partitions[target].len() / 2;
    let order = if source.right {
        [17, fresh, 18]
    } else {
        [fresh, 17, 18]
    };
    let ranges = if source.right {
        [
            partitions[0],
            &partitions[1][..middle],
            &partitions[1][middle..],
        ]
    } else {
        [
            &partitions[0][..middle],
            &partitions[0][middle..],
            partitions[1],
        ]
    };
    for (position, page) in order.into_iter().enumerate() {
        let original = (page != fresh).then(|| &before[page * PAGE..(page + 1) * PAGE]);
        let entries = ranges[position]
            .iter()
            .copied()
            .map(|key| (key, 0))
            .collect::<Vec<_>>();
        let previous = position
            .checked_sub(1)
            .map(|prior| order[prior] as u32)
            .unwrap_or(0);
        let next = order.get(position + 1).copied().unwrap_or(0) as u32;
        install(
            &mut after,
            page,
            &frame(original, &entries, true, (previous, next), 0),
        );
    }
    let bounds = [
        (ranges[0][ranges[0].len() - 1], order[0] as u32),
        (ranges[1][ranges[1].len() - 1], order[1] as u32),
    ];
    install(
        &mut after,
        13,
        &frame(
            Some(&before[13 * PAGE..14 * PAGE]),
            &bounds,
            false,
            (0, 0),
            18,
        ),
    );
    if let Some(bitmap) = bitmap {
        let row = &mut after[14 * PAGE + ROW_START..15 * PAGE];
        row.fill(0);
        row[0] = 1;
        row[1..5].copy_from_slice(&(bitmap as u32).to_le_bytes());
        let mut bytes = vec![0; PAGE];
        bytes[..4].copy_from_slice(&[5, 1, 0, 0]);
        for page in [13, 17, 18, fresh] {
            bytes[4 + page / 8] |= 1 << (page % 8);
        }
        install(&mut after, bitmap, &bytes);
    } else {
        after[14 * PAGE + ROW_START + 5 + fresh / 8] |= 1 << (fresh % 8);
    }
    for page in before.len() / PAGE..after.len() / PAGE {
        let offset = if source.distant {
            599 * PAGE + 4
        } else {
            PAGE + ROW_START + 5
        };
        after[offset + page / 8] &= !(1 << (page % 8));
    }
    after
}

pub(super) fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    if let Some(offset) = actual.iter().zip(expected).position(|(a, b)| a != b) {
        panic!(
            "whole-image mismatch at {offset} (page {}, offset {}): actual {:02x}, expected {:02x}",
            offset / PAGE,
            offset % PAGE,
            actual[offset],
            expected[offset]
        );
    }
}
