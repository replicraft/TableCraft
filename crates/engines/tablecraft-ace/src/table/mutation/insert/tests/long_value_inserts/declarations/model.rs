//! Literal missing declarations, honest logical TD extents, and closed +10 tails.
use crate::table::mutation::insert::tests::{
    long_value_inserts::{Family, initialization::model as zero},
    multiple_indexes::model as frames,
};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const DEFINITION: usize = 10 * PAGE;
const LENGTH_OFFSET: usize = 8;
const FREE_OFFSET: usize = 2;
const LOGICAL_START: usize = 296;
const LOGICAL_BYTES: usize = 28;
const ORIGINAL_LOGICAL_COUNT: usize = 4;
const ORIGINAL_NAMES_START: usize = 408;
const ORIGINAL_TAIL: usize = 498;
const LOGICAL_COUNT_OFFSET: usize = 47;
const TRAILER: [u8; 2] = [255; 2];
const PAIR_BYTES: usize = 10;
const MAX_LOGICAL_INDEXES: usize = 32;
const MAX_NAME_UNITS: usize = 64;
const ALIAS_PREFIX_UNITS: usize = 8;
const NAME_LENGTH_BYTES: usize = 2;
const UTF16_BYTES: usize = 2;
const BASE_ALIAS_BYTES: usize =
    LOGICAL_BYTES + NAME_LENGTH_BYTES + ALIAS_PREFIX_UNITS * UTF16_BYTES;
const MAX_ALIAS_BYTES: usize = LOGICAL_BYTES + NAME_LENGTH_BYTES + MAX_NAME_UNITS * UTF16_BYTES;

pub struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub value: Value,
    pub terminal: usize,
    pub old_length: usize,
    pub new_length: usize,
}

fn put16(bytes: &mut [u8], offset: usize, value: usize) {
    bytes[offset..offset + 2].copy_from_slice(&(value as u16).to_le_bytes());
}

fn put32(bytes: &mut [u8], offset: usize, value: usize) {
    bytes[offset..offset + 4].copy_from_slice(&(value as u32).to_le_bytes());
}

fn extent(bytes: &mut [u8], length: usize) {
    assert!(length <= PAGE);
    put32(bytes, DEFINITION + LENGTH_OFFSET, length);
    put16(
        bytes,
        DEFINITION + FREE_OFFSET,
        PAGE.saturating_sub(length + 8),
    );
}

/// Add genuine logical aliases rather than inventing unparsed padding before
/// the trailer. Every name stays below Access's 64-character user-name limit.
fn aliases(bytes: &mut [u8], terminal: usize) {
    assert!(terminal >= ORIGINAL_TAIL);
    assert_eq!((terminal - ORIGINAL_TAIL) % 2, 0);
    let definition = &mut bytes[DEFINITION..DEFINITION + PAGE];
    let original = definition[LOGICAL_START..ORIGINAL_NAMES_START].to_vec();
    let names = definition[ORIGINAL_NAMES_START..ORIGINAL_TAIL].to_vec();
    let difference = terminal - ORIGINAL_TAIL;
    let added = difference.div_ceil(MAX_ALIAS_BYTES);
    assert!(added <= MAX_LOGICAL_INDEXES - ORIGINAL_LOGICAL_COUNT);
    assert!(difference >= added * BASE_ALIAS_BYTES);
    let mut remaining = difference - added * BASE_ALIAS_BYTES;
    let count = ORIGINAL_LOGICAL_COUNT + added;
    definition[LOGICAL_START..].fill(0);
    definition[LOGICAL_START..ORIGINAL_NAMES_START].copy_from_slice(&original);
    for position in 0..added {
        let start = LOGICAL_START + (ORIGINAL_LOGICAL_COUNT + position) * LOGICAL_BYTES;
        definition[start..start + LOGICAL_BYTES]
            .copy_from_slice(&original[3 * LOGICAL_BYTES..4 * LOGICAL_BYTES]);
        put32(definition, start + 4, ORIGINAL_LOGICAL_COUNT + position);
    }
    put32(definition, LOGICAL_COUNT_OFFSET, count);
    let mut position = LOGICAL_START + count * LOGICAL_BYTES;
    definition[position..position + names.len()].copy_from_slice(&names);
    position += names.len();
    for index in 0..added {
        let mut name = format!("Extra{index:03}");
        let extra_units = (remaining / UTF16_BYTES).min(MAX_NAME_UNITS - ALIAS_PREFIX_UNITS);
        name.push_str(&"X".repeat(extra_units));
        remaining -= extra_units * UTF16_BYTES;
        assert!(name.len() <= MAX_NAME_UNITS);
        let encoded = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        put16(definition, position, encoded.len());
        position += 2;
        definition[position..position + encoded.len()].copy_from_slice(&encoded);
        position += encoded.len();
    }
    assert_eq!(remaining, 0);
    assert!(count <= MAX_LOGICAL_INDEXES);
    assert_eq!(position, terminal);
}

fn pair(bytes: &mut [u8], position: usize, column: u16, owned: u32, candidate: u32) {
    bytes[position..position + 2].copy_from_slice(&column.to_le_bytes());
    bytes[position + 2..position + 6].copy_from_slice(&owned.to_le_bytes());
    bytes[position + 6..position + PAIR_BYTES].copy_from_slice(&candidate.to_le_bytes());
}

pub fn external(code: u8, family: Family, sibling: bool, near_capacity: bool) -> Case {
    let value = match family {
        Family::Ole => Value::Binary(vec![0x92; 65]),
        Family::Memo => Value::Text("Ā".repeat(33)),
        Family::CompressedMemo => Value::Text("A".repeat(65)),
    };
    external_value(code, family, value, sibling, near_capacity)
}

pub fn external_value(
    code: u8,
    family: Family,
    value: Value,
    sibling: bool,
    near_capacity: bool,
) -> Case {
    let original = zero::case(code, family, value);
    let mut before = original.before;
    let mut expected = original.expected;
    let base_terminal = if near_capacity { 4084 } else { ORIGINAL_TAIL };
    assert!(!near_capacity || !sibling);
    aliases(&mut before, base_terminal);
    aliases(&mut expected, base_terminal);
    let terminal = base_terminal + usize::from(sibling) * PAIR_BYTES;
    if sibling {
        for bytes in [&mut before, &mut expected] {
            pair(
                bytes,
                DEFINITION + base_terminal,
                99,
                (11 << 8) | 2,
                (11 << 8) | 3,
            );
        }
    }
    let old_length = terminal + TRAILER.len();
    let new_length = old_length + PAIR_BYTES;
    before[DEFINITION + terminal..DEFINITION + old_length].copy_from_slice(&TRAILER);
    extent(&mut before, old_length);
    pair(
        &mut expected,
        DEFINITION + terminal,
        2,
        18 << 8,
        (18 << 8) | 1,
    );
    expected[DEFINITION + terminal + PAIR_BYTES..DEFINITION + new_length].copy_from_slice(&TRAILER);
    extent(&mut expected, new_length);
    Case {
        before,
        expected,
        value: original.value,
        terminal,
        old_length,
        new_length,
    }
}

pub fn inline(code: u8, family: Family) -> Case {
    let mut case = external(code, family, true, false);
    let mut source = frames::source(code, false, false);
    source.before = case.before.clone();
    source.rows = (1..=3)
        .map(|id| {
            (
                frames::Row {
                    id,
                    group: Some(id * 10),
                    binary: None,
                },
                (12 << 8) | (id as u32 - 1),
            )
        })
        .collect();
    let mut descriptor = 0x80000000_u32.to_le_bytes().to_vec();
    descriptor.extend([0; 8]);
    case.expected = frames::expected(
        &source,
        &[frames::Row {
            id: 10,
            group: Some(100),
            binary: Some(descriptor),
        }],
    );
    // The empty record fits the existing ordinary page; no allocation or map
    // membership changes. Preserve the original mixed container and global map
    // rather than importing the ordinary-only fixture's two-row rewrite.
    assert_eq!(case.expected.len(), case.before.len());
    case.expected[PAGE..2 * PAGE].copy_from_slice(&case.before[PAGE..2 * PAGE]);
    case.expected[11 * PAGE..12 * PAGE].copy_from_slice(&case.before[11 * PAGE..12 * PAGE]);
    case.value = match family {
        Family::Ole => Value::Binary(Vec::new()),
        Family::Memo | Family::CompressedMemo => Value::Text(String::new()),
    };
    case.new_length = case.old_length;
    case
}

/// A valid complete source TD just beyond the one-page extension boundary.
/// UTF-16 names and ten-byte pairs force an even terminal, hence the nearest
/// real extents are 4086->4096 (fits) and 4088->4098 (refuses).
pub fn overflow_source(code: u8, family: Family) -> Vec<u8> {
    let mut bytes = external(code, family, false, false).before;
    aliases(&mut bytes, 4086);
    bytes[DEFINITION + 4086..DEFINITION + 4088].copy_from_slice(&TRAILER);
    extent(&mut bytes, 4088);
    bytes
}

/// Physically unused bytes are qualified by their exact before image, not by
/// their value. Preserve every byte beyond the new logical extent.
pub fn nonzero_spare(code: u8, family: Family) -> Case {
    let mut case = external(code, family, true, false);
    for (position, byte) in case.before[DEFINITION + case.old_length..DEFINITION + case.new_length]
        .iter_mut()
        .enumerate()
    {
        *byte = 0x60 + position as u8;
    }
    for offset in case.new_length..PAGE {
        let byte = (offset as u8).wrapping_mul(17).wrapping_add(0x51);
        case.before[DEFINITION + offset] = byte;
        case.expected[DEFINITION + offset] = byte;
    }
    case
}
