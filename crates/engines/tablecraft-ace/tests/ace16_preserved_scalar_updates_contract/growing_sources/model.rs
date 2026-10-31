//! Independent complete images; no production serializer or allocator is called.
use super::rich;
use std::collections::BTreeSet;

const PAGE: usize = 4096;
const DATA: usize = 12;
const MAP: usize = 11;
const GROW: &[u8] = &[234, 150, 234, 150, 234, 150, 234, 150, 234, 150, 234, 150];
const SHRINK: &[u8] = &[234, 150];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    OriginalPage,
    OtherOwned,
    EndOfFile,
    PreForwarded,
    Mixed,
    MixedReverse,
    Noops,
    ZeroNull,
}

pub(crate) const SHAPES: [Shape; 8] = [
    Shape::OriginalPage,
    Shape::OtherOwned,
    Shape::EndOfFile,
    Shape::PreForwarded,
    Shape::Mixed,
    Shape::MixedReverse,
    Shape::Noops,
    Shape::ZeroNull,
];

pub(crate) struct Request {
    pub row: usize,
    pub expected: Option<&'static str>,
    pub replacement: Option<&'static str>,
}

pub(crate) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub requests: Vec<Request>,
    pub values: [Option<&'static str>; 3],
    pub changed: usize,
    pub destination: Option<(usize, usize)>,
}

fn read16(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn put16(bytes: &mut [u8], at: usize, value: usize) {
    bytes[at..at + 2].copy_from_slice(&u16::try_from(value).unwrap().to_le_bytes());
}

fn record(bytes: &[u8], row: usize) -> Vec<u8> {
    let base = DATA * PAGE;
    let start = read16(bytes, base + 14 + row * 2) & 0x1fff;
    let end = if row == 0 {
        PAGE
    } else {
        read16(bytes, base + 12 + row * 2) & 0x1fff
    };
    bytes[base + start..base + end].to_vec()
}

fn map_bit(bytes: &mut [u8], page: usize, row: usize, member: usize, set: bool) {
    let start = read16(bytes, page * PAGE + 14 + row * 2) & 0x1fff;
    let at = page * PAGE + start + 5 + member / 8;
    let mask = 1 << (member % 8);
    bytes[at] = (bytes[at] & !mask) | if set { mask } else { 0 };
}

fn empty_page() -> Vec<u8> {
    let mut page = vec![0; PAGE];
    page[..2].copy_from_slice(&[1, 1]);
    put16(&mut page, 2, 4082);
    page[4..8].copy_from_slice(&10_u32.to_le_bytes());
    page
}

// Target is native variable zero. Fixed prefix and four opaque sibling payloads
// come from the independently authored original, never decoded placeholders.
fn replacement(before: &[u8], payload: &[u8], present: bool, shorten: bool) -> Vec<u8> {
    assert_eq!(read16(before, 0), 16);
    let count_at = before.len() - 4;
    assert_eq!(read16(before, count_at), 5);
    let offsets = (0..6)
        .map(|position| read16(before, count_at - (position + 1) * 2))
        .collect::<Vec<_>>();
    assert_eq!(offsets[0], 130);
    let end = offsets[5];
    let logical_end = 130 + payload.len() + end - offsets[1];
    let length = if shorten {
        logical_end + 16
    } else {
        before.len().max(logical_end + 16)
    };
    let mut after = before.to_vec();
    after.resize(length, 0);
    after[130..130 + payload.len()].copy_from_slice(payload);
    after[130 + payload.len()..logical_end].copy_from_slice(&before[offsets[1]..end]);
    let new_count_at = length - 4;
    for (position, old) in offsets.into_iter().enumerate() {
        let offset = if position == 0 {
            130
        } else {
            130 + payload.len() + old - offsets_value(before, count_at, 1)
        };
        put16(&mut after, new_count_at - (position + 1) * 2, offset);
    }
    put16(&mut after, new_count_at, 5);
    let mask = read16(before, before.len() - 2);
    put16(
        &mut after,
        length - 2,
        (mask & !2) | if present { 2 } else { 0 },
    );
    after
}

fn offsets_value(before: &[u8], count_at: usize, position: usize) -> usize {
    read16(before, count_at - (position + 1) * 2)
}

fn zero_null_source(before: &mut [u8]) {
    let rows = [record(before, 0), record(before, 1), record(before, 2)];
    let absent = replacement(&rows[1], &[], false, true);
    assert_eq!(absent.len(), 209);
    let mut page = empty_page();
    let mut end = PAGE;
    for (row, body) in [&rows[0][..], &absent, &rows[2]].into_iter().enumerate() {
        end -= body.len();
        page[end..end + body.len()].copy_from_slice(body);
        put16(&mut page, 14 + row * 2, end);
    }
    put16(&mut page, 12, 3);
    put16(&mut page, 2, end - 20);
    before[DATA * PAGE..(DATA + 1) * PAGE].copy_from_slice(&page);
}

// Destination and slot are explicit SOURCE recipe inputs, not allocator outputs.
fn install_hidden(bytes: &mut [u8], destination: usize, slot: usize, body: &[u8]) {
    let base = destination * PAGE;
    assert_eq!(read16(bytes, base + 12), slot);
    let end = if slot == 0 {
        PAGE
    } else {
        read16(bytes, base + 12 + slot * 2) & 0x1fff
    };
    let start = end - body.len();
    bytes[base + start..base + end].copy_from_slice(body);
    put16(bytes, base + 14 + slot * 2, 0x8000 | start);
    put16(bytes, base + 12, slot + 1);
    put16(bytes, base + 2, start - (14 + (slot + 1) * 2));
}

fn forward(bytes: &mut [u8], row: usize, destination: usize, slot: usize) {
    let base = DATA * PAGE;
    let word = read16(bytes, base + 14 + row * 2);
    let start = word & 0x1fff;
    let end = if row == 0 {
        PAGE
    } else {
        read16(bytes, base + 12 + row * 2) & 0x1fff
    };
    if word & 0x4000 == 0 {
        bytes[base + start..base + end].fill(0);
    }
    bytes[base + start..base + start + 4].copy_from_slice(
        &u32::try_from((destination << 8) | slot)
            .unwrap()
            .to_le_bytes(),
    );
    put16(bytes, base + 14 + row * 2, word | 0x4000);
}

pub(crate) fn source(shape: Shape) -> Case {
    let mut before = rich::source().before;
    assert_eq!(before.len(), 32 * PAGE);
    assert_eq!(before[20], 6);
    assert_eq!(
        [
            record(&before, 0).len(),
            record(&before, 1).len(),
            record(&before, 2).len()
        ],
        [219, 211, 265]
    );
    assert_eq!(read16(&before, DATA * PAGE + 2), 3381);
    if shape == Shape::ZeroNull {
        zero_null_source(&mut before);
    }
    let row = usize::from(shape == Shape::ZeroNull);
    let old = record(&before, row);
    if shape == Shape::OtherOwned {
        before.extend(empty_page());
        map_bit(&mut before, MAP, 0, 32, true);
        map_bit(&mut before, MAP, 1, 32, true);
        map_bit(&mut before, 1, 0, 32, false);
    }
    if matches!(shape, Shape::EndOfFile | Shape::ZeroNull) {
        map_bit(&mut before, MAP, 1, 12, false);
    }
    if shape == Shape::PreForwarded {
        install_hidden(&mut before, DATA, 3, &old);
        forward(&mut before, 0, DATA, 3);
    }
    let mut expected = before.clone();
    let mut requests = vec![Request {
        row,
        expected: if row == 1 { None } else { Some("alpha") },
        replacement: Some("雪雪雪雪雪雪"),
    }];
    let mut values = [Some("alpha"), None, Some("omega")];
    if shape == Shape::Noops {
        requests[0].replacement = Some("alpha");
        requests.push(Request {
            row: 1,
            expected: None,
            replacement: None,
        });
        return Case {
            before,
            expected,
            requests,
            values,
            changed: 0,
            destination: None,
        };
    }
    let destination = if matches!(
        shape,
        Shape::OtherOwned | Shape::EndOfFile | Shape::ZeroNull
    ) {
        32
    } else {
        DATA
    };
    let slot = if destination == 32 {
        0
    } else if shape == Shape::PreForwarded {
        4
    } else {
        3
    };
    if matches!(shape, Shape::EndOfFile | Shape::ZeroNull) {
        expected.extend(empty_page());
        map_bit(&mut expected, MAP, 0, 32, true);
        map_bit(&mut expected, MAP, 1, 32, true);
        map_bit(&mut expected, 1, 0, 32, false);
    }
    let grown = replacement(&old, GROW, true, false);
    assert_eq!(grown.len(), 221);
    install_hidden(&mut expected, destination, slot, &grown);
    forward(&mut expected, row, destination, slot);
    values[row] = Some("雪雪雪雪雪雪");
    let mixed = matches!(shape, Shape::Mixed | Shape::MixedReverse);
    if mixed {
        let old = record(&before, 2);
        let shrunk = replacement(&old, SHRINK, true, false);
        let start = read16(&before, DATA * PAGE + 18) & 0x1fff;
        expected[DATA * PAGE + start..DATA * PAGE + start + shrunk.len()].copy_from_slice(&shrunk);
        requests.push(Request {
            row: 1,
            expected: None,
            replacement: None,
        });
        requests.push(Request {
            row: 2,
            expected: Some("omega"),
            replacement: Some("雪"),
        });
        values[2] = Some("雪");
        if shape == Shape::MixedReverse {
            requests.reverse();
        }
    }
    Case {
        before,
        expected,
        requests,
        values,
        changed: if mixed { 2 } else { 1 },
        destination: Some((destination, slot)),
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Contradiction {
    TerminalAlias,
    IndexOwnsParentData,
    GlobalSaysParentFree,
}

pub(crate) fn contradiction(kind: Contradiction) -> Vec<u8> {
    let mut bytes = rich::source().before;
    match kind {
        Contradiction::TerminalAlias => {
            let body = record(&bytes, 0);
            install_hidden(&mut bytes, DATA, 3, &body);
            forward(&mut bytes, 0, DATA, 3);
            forward(&mut bytes, 1, DATA, 3);
        }
        Contradiction::IndexOwnsParentData => map_bit(&mut bytes, 30, 0, 12, true),
        Contradiction::GlobalSaysParentFree => map_bit(&mut bytes, 1, 0, 12, true),
    }
    bytes
}

pub(crate) fn assert_placement(case: &Case, shape: Shape) {
    assert_raw_siblings(case, shape);
    let originals = [Some("alpha"), None, Some("omega")];
    let mut changes = 0;
    for request in &case.requests {
        assert_eq!(request.expected, originals[request.row]);
        assert_eq!(request.replacement, case.values[request.row]);
        changes += usize::from(request.expected != request.replacement);
    }
    assert_eq!(case.changed, changes);

    let row = usize::from(shape == Shape::ZeroNull);
    let root_at = DATA * PAGE + 14 + row * 2;
    let original_word = read16(&case.before, root_at);
    let source_start = original_word & 0x1fff;
    if let Some((page, slot)) = case.destination {
        assert_eq!(read16(&case.expected, root_at), original_word | 0x4000);
        assert_eq!(
            u32::from_le_bytes(
                case.expected[DATA * PAGE + source_start..DATA * PAGE + source_start + 4]
                    .try_into()
                    .unwrap()
            ),
            u32::try_from((page << 8) | slot).unwrap()
        );
        let base = page * PAGE;
        let word = read16(&case.expected, base + 14 + slot * 2);
        assert_eq!(word & 0xe000, 0x8000);
        assert_eq!(read16(&case.expected, base + 12), slot + 1);
        let start = word & 0x1fff;
        let end = if slot == 0 {
            PAGE
        } else {
            read16(&case.expected, base + 12 + slot * 2) & 0x1fff
        };
        assert_eq!(end - start, 221);
        assert_eq!(
            read16(&case.expected, base + 2),
            start - (14 + (slot + 1) * 2)
        );
        assert_eq!(&case.expected[base..base + 2], &[1, 1]);
        assert_eq!(&case.expected[base + 4..base + 8], &10_u32.to_le_bytes());
    }
    let map_offset = |bytes: &[u8], page, row: usize| {
        page * PAGE + (read16(bytes, page * PAGE + 14 + row * 2) & 0x1fff) + 9
    };
    let actual = [1, 11]
        .into_iter()
        .flat_map(|page| {
            let before = &case.before;
            let after = &case.expected;
            (page * PAGE..(page + 1) * PAGE).filter_map(move |at| {
                (before[at] != after[at]).then_some((at, before[at] ^ after[at]))
            })
        })
        .collect::<Vec<_>>();
    if matches!(shape, Shape::EndOfFile | Shape::ZeroNull) {
        assert_eq!(
            actual,
            [
                (map_offset(&case.before, 1, 0), 1),
                (map_offset(&case.before, 11, 1), 1),
                (map_offset(&case.before, 11, 0), 1),
            ]
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
        );
        assert_eq!(
            (case.before.len(), case.expected.len()),
            (32 * PAGE, 33 * PAGE)
        );
    } else {
        assert!(actual.is_empty());
        assert_eq!(case.before.len(), case.expected.len());
    }
}

fn physical_record(bytes: &[u8], page: usize, slot: usize) -> &[u8] {
    let base = page * PAGE;
    let start = read16(bytes, base + 14 + slot * 2) & 0x1fff;
    let end = if slot == 0 {
        PAGE
    } else {
        read16(bytes, base + 12 + slot * 2) & 0x1fff
    };
    &bytes[base + start..base + end]
}

fn terminal_record(bytes: &[u8], row: usize) -> &[u8] {
    let word = read16(bytes, DATA * PAGE + 14 + row * 2);
    let root = physical_record(bytes, DATA, row);
    if word & 0x4000 == 0 {
        root
    } else {
        let pointer = u32::from_le_bytes(root[..4].try_into().unwrap()) as usize;
        let page = pointer >> 8;
        let slot = pointer & 255;
        let terminal_word = read16(bytes, page * PAGE + 14 + slot * 2);
        assert_eq!(terminal_word & 0xe000, 0x8000);
        physical_record(bytes, page, slot)
    }
}

fn record_offsets(bytes: &[u8]) -> [usize; 6] {
    let count_at = bytes.len() - 4;
    assert_eq!(read16(bytes, 0), 16);
    assert_eq!(read16(bytes, count_at), 5);
    std::array::from_fn(|position| read16(bytes, count_at - (position + 1) * 2))
}

fn assert_raw_siblings(case: &Case, shape: Shape) {
    for row in 0..3 {
        let before = terminal_record(&case.before, row);
        let after = terminal_record(&case.expected, row);
        let request = case.requests.iter().find(|request| request.row == row);
        let changed = request.is_some_and(|request| request.expected != request.replacement);
        if !changed {
            assert_eq!(before, after);
            continue;
        }
        assert_eq!(&before[..130], &after[..130]);
        let before_offsets = record_offsets(before);
        let after_offsets = record_offsets(after);
        for variable in 1..5 {
            assert_eq!(
                &before[before_offsets[variable]..before_offsets[variable + 1]],
                &after[after_offsets[variable]..after_offsets[variable + 1]],
                "opaque sibling row {row} variable {variable}"
            );
        }
        let before_mask = read16(before, before.len() - 2);
        let after_mask = read16(after, after.len() - 2);
        assert_eq!(before_mask & !2, after_mask & !2);
    }
    if shape == Shape::PreForwarded {
        // The source terminal is abandoned, not rewritten into another link.
        assert_eq!(
            physical_record(&case.before, DATA, 3),
            physical_record(&case.expected, DATA, 3)
        );
        assert_eq!(
            read16(&case.before, DATA * PAGE + 20),
            read16(&case.expected, DATA * PAGE + 20)
        );
    }
}
