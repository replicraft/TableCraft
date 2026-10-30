//! Independent C3/V0 nullable nonprimary fields and complete fixed UPDATE images.
//! Draft child of the adapter test owner, using its actual fixture/support imports.
use super::{fixture, support};

const PAGE: usize = 4096;

type Fields = [Option<i32>; 2];

#[derive(Clone, Copy)]
pub(super) enum Recipe {
    Clear,
    Exchange,
    Introduce,
}

pub(super) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub original: [Fields; 3],
    pub final_values: [Fields; 3],
}

fn map(members: impl IntoIterator<Item = u32>) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for member in members {
        bytes[5 + member as usize / 8] |= 1 << (member % 8);
    }
    bytes
}

fn component(value: Option<i32>, descending: bool) -> Vec<u8> {
    let mut bytes = match value {
        Some(value) => {
            let mut bytes = vec![127];
            bytes.extend(((value as u32) ^ 0x8000_0000).to_be_bytes());
            bytes
        }
        None => vec![0],
    };
    if descending {
        for byte in &mut bytes {
            *byte = !*byte;
        }
    }
    bytes
}

fn leaf(values: &[Fields; 3], tuple: bool, original: Option<&[u8]>) -> Vec<u8> {
    let mut entries = Vec::new();
    for (ordinal, fields) in values.iter().enumerate() {
        if tuple && fields == &[None, None] {
            continue;
        }
        let mut key = if tuple {
            let mut key = component(fields[1], true);
            key.extend(component(fields[0], false));
            key
        } else {
            component(fields[0], false)
        };
        let key_length = key.len();
        key.extend(((12 << 8) | ordinal as u32).to_be_bytes());
        entries.push((key, key_length));
    }
    entries.sort_by(|left, right| {
        (&left.0[..left.1], &left.0[left.1..]).cmp(&(&right.0[..right.1], &right.0[right.1..]))
    });
    // This deliberately chooses the valid uncompressed framing for both SOURCE
    // and expected: three entries never need allocation or a common prefix.
    let mut page = original.map_or_else(|| vec![0; PAGE], <[u8]>::to_vec);
    page[..480].fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    support::put32(&mut page, 4, 10);
    let mut end = 0;
    for (wire, _) in entries {
        page[480 + end..480 + end + wire.len()].copy_from_slice(&wire);
        end += wire.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(&mut page, 2, (3616 - end) as u16);
    page
}

fn finish_definition(page: &mut [u8], tail: usize) {
    support::put16(page, tail, 0xffff);
    support::put32(page, 8, (tail + 2) as u32);
    support::put16(page, 2, (PAGE - tail - 2 - 8) as u16);
}

pub(super) fn case(code: u8, recipe: Recipe) -> Case {
    let original = [[Some(10), Some(100)], [Some(10), None], [None, None]];
    let final_values = match recipe {
        Recipe::Clear => [[None, None], [Some(10), None], [None, None]],
        Recipe::Exchange => [[Some(100), Some(10)], [None, Some(10)], [None, None]],
        Recipe::Introduce => [
            [Some(10), Some(100)],
            [Some(10), None],
            [Some(20), Some(200)],
        ],
    };
    let mut before = fixture::with_rule("Caption", "nullable SQL tuple source");
    before.resize(16 * PAGE, 0);
    before[4..20].copy_from_slice(b"Standard ACE DB\0");
    before[20] = code;
    let mut definition = fixture::definition(
        &[("Id", 4, 0, 4), ("Group", 4, 4, 4), ("Score", 4, 8, 4)],
        3,
        11 << 8,
    );
    // Two physical owner history blocks precede the genuine fixed descriptors.
    let names_end = 63 + 75 + 2 + 4 + 2 + 10 + 2 + 10;
    definition.copy_within(63..names_end, 87);
    definition[63..87].fill(0);
    support::put32(&mut definition, 67, 17);
    support::put32(&mut definition, 79, 29);
    support::put32(&mut definition, 47, 4);
    support::put32(&mut definition, 51, 2);
    support::put32(&mut definition, 59, (11 << 8) | 1);
    let physical = names_end + 24;
    definition[physical..].fill(0);
    for owner in 0..2 {
        let offset = physical + owner * 52;
        for component in 0..10 {
            support::put16(&mut definition, offset + 4 + component * 3, 0xffff);
        }
        support::put16(&mut definition, offset + 4, if owner == 0 { 1 } else { 2 });
        definition[offset + 6] = u8::from(owner == 0);
        if owner == 1 {
            support::put16(&mut definition, offset + 7, 1);
            definition[offset + 9] = 1;
        }
        support::put32(&mut definition, offset + 34, (15 << 8) | owner as u32);
        support::put32(&mut definition, offset + 38, 13 + owner as u32);
        definition[offset + 46] = if owner == 0 { 0x80 } else { 0x82 };
    }
    let logical = physical + 104;
    let mut tail = logical + 112;
    for (position, name) in ["GroupOrder", "GroupAlias", "PairOrder", "PairAlias"]
        .iter()
        .enumerate()
    {
        let offset = logical + position * 28;
        support::put32(&mut definition, offset + 4, position as u32);
        support::put32(&mut definition, offset + 8, (position / 2) as u32);
        let name = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        support::put16(&mut definition, tail, name.len() as u16);
        tail += 2;
        definition[tail..tail + name.len()].copy_from_slice(&name);
        tail += name.len();
    }
    finish_definition(&mut definition, tail);
    before[10 * PAGE..11 * PAGE].copy_from_slice(&definition);
    let records = original
        .iter()
        .enumerate()
        .map(|(ordinal, fields)| {
            let mut record = 3_u16.to_le_bytes().to_vec();
            record.extend((ordinal as i32 + 1).to_le_bytes());
            record.extend(fields[0].map_or([0xa5; 4], i32::to_le_bytes));
            record.extend(fields[1].map_or([0x5a; 4], i32::to_le_bytes));
            record.push(
                1 | (u8::from(fields[0].is_some()) << 1) | (u8::from(fields[1].is_some()) << 2),
            );
            (0, record)
        })
        .collect::<Vec<_>>();
    let mut data = support::data_page(10, &records);
    support::put16(&mut data, 2, 4031);
    before[12 * PAGE..13 * PAGE].copy_from_slice(&data);
    before[PAGE..2 * PAGE].copy_from_slice(&support::data_page(1, &[(0, map(16..512))]));
    before[11 * PAGE..12 * PAGE]
        .copy_from_slice(&support::data_page(0, &[(0, map([12])), (0, map([12]))]));
    before[15 * PAGE..16 * PAGE]
        .copy_from_slice(&support::data_page(0, &[(0, map([13])), (0, map([14]))]));
    for number in [2, 7] {
        let page = &mut before[number * PAGE..(number + 1) * PAGE];
        let columns = usize::from(u16::from_le_bytes(page[45..47].try_into().unwrap()));
        let mut tail = 63 + columns * 25;
        for _ in 0..columns {
            tail += 2 + usize::from(u16::from_le_bytes(page[tail..tail + 2].try_into().unwrap()));
        }
        finish_definition(page, tail);
    }
    before[13 * PAGE..14 * PAGE].copy_from_slice(&leaf(&original, false, None));
    before[14 * PAGE..15 * PAGE].copy_from_slice(&leaf(&original, true, None));
    let mut expected = before.clone();
    for (ordinal, fields) in final_values.iter().enumerate() {
        let directory = 12 * PAGE + 14 + ordinal * 2;
        let start = 12 * PAGE
            + usize::from(u16::from_le_bytes(
                expected[directory..directory + 2].try_into().unwrap(),
            ));
        for component in 0..2 {
            if original[ordinal][component] != fields[component] {
                if let Some(value) = fields[component] {
                    expected[start + 6 + component * 4..start + 10 + component * 4]
                        .copy_from_slice(&value.to_le_bytes());
                }
                let mask = 1 << (component + 1);
                expected[start + 14] = (expected[start + 14] & !mask)
                    | if fields[component].is_some() { mask } else { 0 };
            }
        }
    }
    expected[13 * PAGE..14 * PAGE].copy_from_slice(&leaf(
        &final_values,
        false,
        Some(&before[13 * PAGE..14 * PAGE]),
    ));
    expected[14 * PAGE..15 * PAGE].copy_from_slice(&leaf(
        &final_values,
        true,
        Some(&before[14 * PAGE..15 * PAGE]),
    ));
    if matches!(recipe, Recipe::Introduce) {
        support::put32(&mut expected, 10 * PAGE + 79, 30);
    }
    Case {
        before,
        expected,
        original,
        final_values,
    }
}
