//! Private literal source/oracle. No production key encoder or candidate bytes.
//! Future parent supplies read-only encoded_model and support fixture modules.
use super::{encoded_model, support};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const ENTRY_START: usize = 480;
const CAPACITY: usize = PAGE - ENTRY_START;
const ROOT: usize = 13;
const EOF: usize = 24;
const ROWS: usize = 900;
const PER_PAGE: usize = 180;
const DATA: [usize; 5] = [12, 15, 16, 17, 18];

#[derive(Clone, Copy, Debug)]
pub enum Family {
    Legacy,
    Modern,
    Binary,
}

pub struct Model {
    pub bytes: Vec<u8>,
    pub expected: Vec<u8>,
    pub records: Vec<Vec<u8>>,
    pub old_key: Vec<u8>,
    pub new_key: Vec<u8>,
    pub leaf_order: [usize; 3],
    pub leaf_counts: [usize; 3],
}

pub struct Case {
    pub bytes: Vec<u8>,
    pub expected: Vec<u8>,
    pub row: usize,
    pub old: Value,
    pub new: Value,
}

pub fn root_case(family: Family, descending: bool) -> Case {
    let model = source(family, descending, true);
    assert_eq!(model.records.len(), ROWS);
    assert_eq!(model.old_key, keys(family, descending).0);
    assert_eq!(model.new_key, keys(family, descending).1);
    assert_eq!(model.leaf_counts.iter().sum::<usize>(), ROWS);
    assert_eq!(model.leaf_order[2], EOF);
    Case {
        bytes: model.bytes,
        expected: model.expected,
        row: 0,
        old: if matches!(family, Family::Binary) {
            Value::Binary(vec![0])
        } else {
            Value::Text("aaa".into())
        },
        new: if matches!(family, Family::Binary) {
            Value::Binary(vec![255])
        } else {
            Value::Text("zzz".into())
        },
    }
}

pub fn small_source(family: Family, descending: bool) -> encoded_model::Model {
    let mut model = match family {
        Family::Legacy => encoded_model::text(0, descending, true),
        Family::Modern => encoded_model::text(1, descending, true),
        Family::Binary => encoded_model::binary(descending, true),
    };
    model.bytes[10 * PAGE + 115] = 2;
    model
}

pub fn binary_key(value: &[u8], descending: bool) -> Vec<u8> {
    let mut key = vec![if descending { 128 } else { 127 }];
    for (ordinal, chunk) in value.chunks(8).enumerate() {
        let mut segment = [if descending { 255 } else { 0 }; 9];
        for (destination, byte) in segment.iter_mut().zip(chunk) {
            *destination = if descending { !byte } else { *byte };
        }
        segment[8] = if (ordinal + 1) * 8 < value.len() {
            9
        } else if descending {
            !(chunk.len() as u8)
        } else {
            chunk.len() as u8
        };
        key.extend(segment);
    }
    key
}

pub fn small_case(
    model: encoded_model::Model,
    row: usize,
    old: Value,
    new: Value,
    new_key: Vec<u8>,
) -> Case {
    let mut expected = model.bytes.clone();
    let start = u16::from_le_bytes(
        expected[12 * PAGE + 14 + row * 2..12 * PAGE + 16 + row * 2]
            .try_into()
            .unwrap(),
    ) as usize;
    let span = model.records[row].len() - 13;
    let payload = match &new {
        Value::Null => None,
        Value::Text(text) => Some(
            text.encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        ),
        Value::Binary(bytes) => Some(bytes.clone()),
        _ => panic!("only actual slot carriers"),
    };
    if let Some(payload) = payload {
        assert_eq!(payload.len(), span);
        expected[12 * PAGE + start + 6..12 * PAGE + start + 6 + span].copy_from_slice(&payload);
        expected[12 * PAGE + start + model.records[row].len() - 1] |= 2;
    } else {
        expected[12 * PAGE + start + model.records[row].len() - 1] &= !2;
    }
    let mut entries = model
        .keys
        .iter()
        .enumerate()
        .map(|(ordinal, key)| {
            let key = if ordinal == row { &new_key } else { key };
            [
                key.as_slice(),
                &((12_u32 << 8) | ordinal as u32).to_be_bytes(),
            ]
            .concat()
        })
        .collect::<Vec<_>>();
    sort_entries(&mut entries);
    let before = &model.bytes[ROOT * PAGE..(ROOT + 1) * PAGE];
    let seed = u16::from_le_bytes(before[24..26].try_into().unwrap()) as usize;
    expected[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&page(
        Some(before),
        &entries,
        false,
        seed,
        0,
        0,
        0,
    ));
    Case {
        bytes: model.bytes,
        expected,
        row,
        old,
        new,
    }
}

pub fn uniform_case(family: Family, descending: bool, width: usize, null: bool) -> Case {
    let mut seed = small_source(family, descending);
    seed.bytes.resize(EOF * PAGE, 0);
    let binary = matches!(family, Family::Binary);
    let old_payload = if binary {
        vec![0; width]
    } else {
        [b'a', 0].repeat(width)
    };
    let new_payload = if binary {
        vec![255; width]
    } else {
        old_payload.clone()
    };
    let old_key = if binary {
        binary_key(&old_payload, descending)
    } else {
        seed.keys[8].clone()
    };
    let new_key = if null {
        vec![if descending { 255 } else { 0 }]
    } else {
        binary_key(&new_payload, descending)
    };
    let per_page = if old_payload.len() > 255 { 7 } else { 8 };
    let rows = 16;
    let mut records = Vec::new();
    let mut references = Vec::new();
    for row in 0..rows {
        let mut record = vec![2, 0];
        record.extend((row as i32 + 1).to_le_bytes());
        record.extend(&old_payload);
        record.extend((record.len() as u16).to_le_bytes());
        record.extend(6_u16.to_le_bytes());
        record.extend(1_u16.to_le_bytes());
        record.push(3);
        records.push(record);
        references.push(((DATA[row / per_page] as u32) << 8) | (row % per_page) as u32);
    }
    let mut owned = vec![0; 69];
    for (block, rows) in records.chunks(per_page).enumerate() {
        let number = DATA[block];
        let rows = rows
            .iter()
            .cloned()
            .map(|record| (0, record))
            .collect::<Vec<_>>();
        seed.bytes[number * PAGE..(number + 1) * PAGE]
            .copy_from_slice(&support::data_page(10, &rows));
        owned[5 + number / 8] |= 1 << (number % 8);
    }
    seed.bytes[11 * PAGE..12 * PAGE].copy_from_slice(&support::data_page(0, &[(0, owned)]));
    support::put32(&mut seed.bytes, 10 * PAGE + 16, rows as u32);
    support::put32(&mut seed.bytes, 10 * PAGE + 67, rows as u32);
    let mut index_owned = vec![0; 69];
    index_owned[5 + ROOT / 8] |= 1 << (ROOT % 8);
    seed.bytes[14 * PAGE..15 * PAGE].copy_from_slice(&support::data_page(0, &[(0, index_owned)]));
    let mut global = vec![0; 69];
    for number in EOF..512 {
        global[5 + number / 8] |= 1 << (number % 8);
    }
    seed.bytes[PAGE..2 * PAGE].copy_from_slice(&support::data_page(1, &[(0, global)]));
    let original_entries = references
        .iter()
        .map(|reference| [old_key.as_slice(), &reference.to_be_bytes()].concat())
        .collect::<Vec<_>>();
    let mut original = vec![0xa7; PAGE];
    original[..ENTRY_START].fill(0);
    let old_root = page(
        Some(&original),
        &original_entries,
        false,
        common(&original_entries, false),
        0,
        0,
        0,
    );
    seed.bytes[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&old_root);
    let mut expected = seed.bytes.clone();
    let start =
        u16::from_le_bytes(expected[12 * PAGE + 14..12 * PAGE + 16].try_into().unwrap()) as usize;
    if null {
        expected[12 * PAGE + start + records[0].len() - 1] &= !2;
    } else {
        expected[12 * PAGE + start + 6..12 * PAGE + start + 6 + new_payload.len()]
            .copy_from_slice(&new_payload);
    }
    let mut entries = references
        .iter()
        .enumerate()
        .map(|(row, reference)| {
            [
                if row == 0 {
                    new_key.as_slice()
                } else {
                    old_key.as_slice()
                },
                &reference.to_be_bytes(),
            ]
            .concat()
        })
        .collect::<Vec<_>>();
    sort_entries(&mut entries);
    let original_prefix = common(&original_entries, false);
    let best = common(&entries, false);
    let minimum = entries.iter().map(Vec::len).sum::<usize>() - best * (rows - 1);
    if minimum <= CAPACITY {
        expected[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&page(
            Some(&old_root),
            &entries,
            false,
            original_prefix,
            0,
            0,
            0,
        ));
    } else {
        expected.resize((EOF + 2) * PAGE, 0);
        let middle = rows / 2;
        let left = page(None, &entries[..middle], false, 0, 0, EOF, 0);
        let right = page(None, &entries[middle..], false, 0, EOF + 1, 0, 0);
        expected[EOF * PAGE..(EOF + 1) * PAGE].copy_from_slice(&right);
        expected[(EOF + 1) * PAGE..(EOF + 2) * PAGE].copy_from_slice(&left);
        let mut bound = entries[middle - 1].clone();
        bound.extend(((EOF + 1) as u32).to_be_bytes());
        let root = page(Some(&old_root), &[bound], true, 0, 0, 0, EOF);
        expected[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&root);
        for number in EOF..EOF + 2 {
            expected[14 * PAGE + 4027 + 5 + number / 8] |= 1 << (number % 8);
            expected[PAGE + 4027 + 5 + number / 8] &= !(1 << (number % 8));
        }
    }
    Case {
        bytes: seed.bytes,
        expected,
        row: 0,
        old: if binary {
            Value::Binary(old_payload)
        } else {
            Value::Text("a".repeat(width))
        },
        new: if null {
            Value::Null
        } else {
            Value::Binary(new_payload)
        },
    }
}

pub fn nonroot_case(family: Family, descending: bool, height_two: bool) -> Case {
    let model = source(family, descending, true);
    let mut bytes = model.bytes;
    let null_row = ROWS - 1;
    let page_number = DATA[null_row / PER_PAGE];
    let slot = null_row % PER_PAGE;
    let begin = u16::from_le_bytes(
        bytes[page_number * PAGE + 14 + slot * 2..page_number * PAGE + 16 + slot * 2]
            .try_into()
            .unwrap(),
    ) as usize;
    bytes[page_number * PAGE + begin + model.records[null_row].len() - 1] &= !2;
    let (old, new) = keys(family, descending);
    let null = vec![if descending { 255 } else { 0 }];
    let mut before_entries = (0..ROWS)
        .map(|row| wire(if row == null_row { &null } else { &old }, row))
        .collect::<Vec<_>>();
    sort_entries(&mut before_entries);
    let counts = if descending {
        [450, 225, 225]
    } else {
        [225, 225, 450]
    };
    let leaves = [19, 20, 21];
    let internal = if height_two { 23 } else { ROOT };
    let mut cursor = 0;
    let mut bounds = Vec::new();
    for (position, (&number, &count)) in leaves.iter().zip(&counts).enumerate() {
        let subset = &before_entries[cursor..cursor + count];
        let mut original = vec![0xa7; PAGE];
        original[..ENTRY_START].fill(0);
        let leaf = page(
            Some(&original),
            subset,
            false,
            common(subset, false),
            if position == 0 {
                0
            } else {
                leaves[position - 1]
            },
            leaves.get(position + 1).copied().unwrap_or(0),
            0,
        );
        bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(&leaf);
        if position < 2 {
            let mut bound = subset.last().unwrap().clone();
            bound.extend((number as u32).to_be_bytes());
            bounds.push(bound);
        }
        cursor += count;
    }
    let mut original = vec![0xa7; PAGE];
    original[..ENTRY_START].fill(0);
    let parent = page(Some(&original), &bounds, true, 0, 0, 0, 21);
    bytes[internal * PAGE..(internal + 1) * PAGE].copy_from_slice(&parent);
    if height_two {
        let mut bound = before_entries.last().unwrap().clone();
        bound.extend(23_u32.to_be_bytes());
        let mut root = page(Some(&original), &[bound], true, 0, 0, 0, 0);
        root[26] = 2;
        bytes[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&root);
    }
    let mut owned = vec![0; 69];
    for number in [ROOT, 19, 20, 21]
        .into_iter()
        .chain(height_two.then_some(23))
    {
        owned[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[14 * PAGE..15 * PAGE].copy_from_slice(&support::data_page(0, &[(0, owned)]));
    let mut expected = bytes.clone();
    expected.resize((EOF + 1) * PAGE, 0);
    let start =
        u16::from_le_bytes(bytes[12 * PAGE + 14..12 * PAGE + 16].try_into().unwrap()) as usize;
    let replacement = if matches!(family, Family::Binary) {
        vec![255]
    } else {
        b"z\0z\0z\0".to_vec()
    };
    expected[12 * PAGE + start + 6..12 * PAGE + start + 6 + replacement.len()]
        .copy_from_slice(&replacement);
    let mut final_entries = (0..ROWS)
        .map(|row| {
            wire(
                if row == null_row {
                    &null
                } else if row == 0 {
                    &new
                } else {
                    &old
                },
                row,
            )
        })
        .collect::<Vec<_>>();
    sort_entries(&mut final_entries);
    let final_pages = if descending {
        [EOF, 19, 20, 21]
    } else {
        [19, 20, EOF, 21]
    };
    let mut bounds = Vec::new();
    for (position, &number) in final_pages.iter().enumerate() {
        let subset = &final_entries[position * 225..(position + 1) * 225];
        let original = if number == EOF {
            None
        } else {
            Some(&bytes[number * PAGE..(number + 1) * PAGE])
        };
        let seed = original.map_or(0, |raw| {
            u16::from_le_bytes(raw[24..26].try_into().unwrap()) as usize
        });
        let leaf = page(
            original,
            subset,
            false,
            seed,
            if position == 0 {
                0
            } else {
                final_pages[position - 1]
            },
            final_pages.get(position + 1).copied().unwrap_or(0),
            0,
        );
        expected[number * PAGE..(number + 1) * PAGE].copy_from_slice(&leaf);
        if position < 3 {
            let mut bound = subset.last().unwrap().clone();
            bound.extend((number as u32).to_be_bytes());
            bounds.push(bound);
        }
    }
    let parent = page(
        Some(&bytes[internal * PAGE..(internal + 1) * PAGE]),
        &bounds,
        true,
        0,
        0,
        0,
        21,
    );
    expected[internal * PAGE..(internal + 1) * PAGE].copy_from_slice(&parent);
    if height_two {
        let mut bound = final_entries.last().unwrap().clone();
        bound.extend(23_u32.to_be_bytes());
        let mut root = page(
            Some(&bytes[ROOT * PAGE..(ROOT + 1) * PAGE]),
            &[bound],
            true,
            0,
            0,
            0,
            0,
        );
        root[26] = 2;
        expected[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&root);
    }
    expected[14 * PAGE + 4027 + 5 + EOF / 8] |= 1 << (EOF % 8);
    expected[PAGE + 4027 + 5 + EOF / 8] &= !(1 << (EOF % 8));
    Case {
        bytes,
        expected,
        row: 0,
        old: if matches!(family, Family::Binary) {
            Value::Binary(vec![0])
        } else {
            Value::Text("aaa".into())
        },
        new: if matches!(family, Family::Binary) {
            Value::Binary(vec![255])
        } else {
            Value::Text("zzz".into())
        },
    }
}

pub fn two_owners(family: Family) -> Case {
    let mut bytes = source(family, false, true).bytes;
    let original_definition = bytes[10 * PAGE..11 * PAGE].to_vec();
    let definition = &mut bytes[10 * PAGE..11 * PAGE];
    definition[63..].fill(0);
    definition[87..155].copy_from_slice(&original_definition[75..143]);
    support::put32(definition, 47, 3);
    support::put32(definition, 51, 2);
    support::put32(definition, 67, ROWS as u32);
    support::put32(definition, 79, ROWS as u32);
    for (physical, root, map, descending) in [(0, ROOT, 14, false), (1, 19, 20, true)] {
        let begin = 155 + physical * 52;
        definition[begin..begin + 52].copy_from_slice(&original_definition[143..195]);
        definition[begin + 6] = u8::from(!descending);
        support::put32(definition, begin + 34, (map << 8) as u32);
        support::put32(definition, begin + 38, root as u32);
    }
    let mut name_offset = 259 + 3 * 28;
    for (logical, physical, name) in [(0, 0, "Ascending"), (1, 0, "Twin"), (2, 1, "Descending")] {
        let begin = 259 + logical * 28;
        definition[begin..begin + 28].copy_from_slice(&original_definition[195..223]);
        support::put32(definition, begin + 4, logical as u32);
        support::put32(definition, begin + 8, physical as u32);
        let name = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        support::put16(definition, name_offset, name.len() as u16);
        name_offset += 2;
        definition[name_offset..name_offset + name.len()].copy_from_slice(&name);
        name_offset += name.len();
    }
    support::put16(definition, name_offset, 0xffff);
    let mut expected = bytes.clone();
    expected.resize((EOF + 6) * PAGE, 0);
    for (physical, root, map, descending) in [(0, ROOT, 14, false), (1, 19, 20, true)] {
        let (old, new) = keys(family, descending);
        let before_entries = (0..ROWS).map(|row| wire(&old, row)).collect::<Vec<_>>();
        let mut original = vec![0xa7; PAGE];
        original[..ENTRY_START].fill(0);
        let old_root = page(
            Some(&original),
            &before_entries,
            false,
            common(&before_entries, false),
            0,
            0,
            0,
        );
        bytes[root * PAGE..(root + 1) * PAGE].copy_from_slice(&old_root);
        let mut owned = vec![0; 69];
        owned[5 + root / 8] |= 1 << (root % 8);
        let map_page = support::data_page(0, &[(0, owned)]);
        bytes[map * PAGE..(map + 1) * PAGE].copy_from_slice(&map_page);
        expected[map * PAGE..(map + 1) * PAGE].copy_from_slice(&map_page);
        let mut entries = (0..ROWS)
            .map(|row| wire(if row == 0 { &new } else { &old }, row))
            .collect::<Vec<_>>();
        sort_entries(&mut entries);
        let base = EOF + physical * 3;
        let (pages, counts) = if descending {
            ([base + 2, base + 1, base], [225, 225, 450])
        } else {
            ([base + 1, base + 2, base], [450, 225, 225])
        };
        let mut position = 0;
        let mut bounds = Vec::new();
        for (ordinal, (&number, &count)) in pages.iter().zip(&counts).enumerate() {
            let subset = &entries[position..position + count];
            let leaf = page(
                None,
                subset,
                false,
                0,
                if ordinal == 0 { 0 } else { pages[ordinal - 1] },
                pages.get(ordinal + 1).copied().unwrap_or(0),
                0,
            );
            expected[number * PAGE..(number + 1) * PAGE].copy_from_slice(&leaf);
            if ordinal < 2 {
                let mut bound = subset.last().unwrap().clone();
                bound.extend((number as u32).to_be_bytes());
                bounds.push(bound);
            }
            position += count;
        }
        let node = page(Some(&old_root), &bounds, true, 0, 0, 0, base);
        expected[root * PAGE..(root + 1) * PAGE].copy_from_slice(&node);
        for number in base..base + 3 {
            expected[map * PAGE + 4027 + 5 + number / 8] |= 1 << (number % 8);
            expected[PAGE + 4027 + 5 + number / 8] &= !(1 << (number % 8));
        }
    }
    let start =
        u16::from_le_bytes(bytes[12 * PAGE + 14..12 * PAGE + 16].try_into().unwrap()) as usize;
    let replacement = if matches!(family, Family::Binary) {
        vec![255]
    } else {
        b"z\0z\0z\0".to_vec()
    };
    expected[12 * PAGE + start + 6..12 * PAGE + start + 6 + replacement.len()]
        .copy_from_slice(&replacement);
    Case {
        bytes,
        expected,
        row: 0,
        old: if matches!(family, Family::Binary) {
            Value::Binary(vec![0])
        } else {
            Value::Text("aaa".into())
        },
        new: if matches!(family, Family::Binary) {
            Value::Binary(vec![255])
        } else {
            Value::Text("zzz".into())
        },
    }
}

pub fn maximum_parent_case() -> Case {
    let base = uniform_case(Family::Modern, false, 255, true);
    let mut bytes = base.bytes;
    bytes.resize(56 * PAGE, 0);
    let old_key = small_source(Family::Modern, false).keys[8].clone();
    assert_eq!(old_key.len(), 510);
    let mut original = vec![0xa7; PAGE];
    original[..ENTRY_START].fill(0);
    let mut before_entries = Vec::new();
    let mut references = Vec::new();
    for row in 0..16 {
        let reference = ((DATA[row / 7] as u32) << 8) | (row % 7) as u32;
        references.push(reference);
        let entry = [old_key.as_slice(), &reference.to_be_bytes()].concat();
        let number = 32 + row;
        let leaf = page(
            Some(&original),
            std::slice::from_ref(&entry),
            false,
            entry.len(),
            if row == 0 { 0 } else { number - 1 },
            if row == 15 { 0 } else { number + 1 },
            0,
        );
        bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(&leaf);
        let mut bound = entry;
        bound.extend((number as u32).to_be_bytes());
        before_entries.push(bound);
    }
    let parent = page(
        Some(&original),
        &before_entries,
        true,
        common(&before_entries, true),
        0,
        0,
        0,
    );
    bytes[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&parent);
    let mut owned = vec![0; 69];
    for number in std::iter::once(ROOT).chain(32..48) {
        owned[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[14 * PAGE..15 * PAGE].copy_from_slice(&support::data_page(0, &[(0, owned)]));
    let mut global = vec![0; 69];
    for number in 56..512 {
        global[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[PAGE..2 * PAGE].copy_from_slice(&support::data_page(1, &[(0, global)]));
    let mut expected = bytes.clone();
    expected.resize(58 * PAGE, 0);
    let row_start =
        u16::from_le_bytes(bytes[12 * PAGE + 14..12 * PAGE + 16].try_into().unwrap()) as usize;
    expected[12 * PAGE + row_start + 522] &= !2;
    let entries = references
        .iter()
        .enumerate()
        .map(|(row, reference)| {
            [
                if row == 0 {
                    &[0][..]
                } else {
                    old_key.as_slice()
                },
                &reference.to_be_bytes(),
            ]
            .concat()
        })
        .collect::<Vec<_>>();
    for (row, entry) in entries.iter().enumerate() {
        let number = 32 + row;
        let before = &bytes[number * PAGE..(number + 1) * PAGE];
        let leaf = page(
            Some(before),
            std::slice::from_ref(entry),
            false,
            u16::from_le_bytes(before[24..26].try_into().unwrap()) as usize,
            if row == 0 { 0 } else { number - 1 },
            if row == 15 { 0 } else { number + 1 },
            0,
        );
        expected[number * PAGE..(number + 1) * PAGE].copy_from_slice(&leaf);
    }
    let bounds = |range: std::ops::Range<usize>| {
        range
            .map(|row| {
                let mut bound = entries[row].clone();
                bound.extend(((32 + row) as u32).to_be_bytes());
                bound
            })
            .collect::<Vec<_>>()
    };
    let left = page(None, &bounds(0..7), true, 0, 0, 56, 39);
    let right = page(None, &bounds(8..15), true, 0, 57, 0, 47);
    expected[56 * PAGE..57 * PAGE].copy_from_slice(&right);
    expected[57 * PAGE..58 * PAGE].copy_from_slice(&left);
    let mut root_entries = Vec::new();
    for (row, child) in [(7, 57_u32), (15, 56_u32)] {
        let mut bound = entries[row].clone();
        bound.extend(child.to_be_bytes());
        root_entries.push(bound);
    }
    let mut root = page(Some(&parent), &root_entries, true, 0, 0, 0, 0);
    root[26] = 2;
    expected[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&root);
    for number in 56..58 {
        expected[14 * PAGE + 4027 + 5 + number / 8] |= 1 << (number % 8);
        expected[PAGE + 4027 + 5 + number / 8] &= !(1 << (number % 8));
    }
    Case {
        bytes,
        expected,
        row: 0,
        old: base.old,
        new: Value::Null,
    }
}

pub fn rewrite_small_source(
    model: &mut encoded_model::Model,
    row: usize,
    payload: &[u8],
    key: Vec<u8>,
    present: bool,
) {
    let mut record = model.records[row][..6].to_vec();
    record.extend(payload);
    record.extend((record.len() as u16).to_le_bytes());
    record.extend(6_u16.to_le_bytes());
    record.extend(1_u16.to_le_bytes());
    record.push(if present { 3 } else { 1 });
    model.records[row] = record;
    model.keys[row] = key;
    let rows = model
        .records
        .iter()
        .cloned()
        .map(|record| (0, record))
        .collect::<Vec<_>>();
    model.bytes[12 * PAGE..13 * PAGE].copy_from_slice(&support::data_page(10, &rows));
    let mut entries = model
        .keys
        .iter()
        .enumerate()
        .map(|(row, key)| [key.as_slice(), &((12_u32 << 8) | row as u32).to_be_bytes()].concat())
        .collect::<Vec<_>>();
    sort_entries(&mut entries);
    let mut original = vec![0xa7; PAGE];
    original[..ENTRY_START].fill(0);
    model.bytes[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&page(
        Some(&original),
        &entries,
        false,
        0,
        0,
        0,
        0,
    ));
}

pub fn property_source(key: &str, native_type: u8, value: &[u8]) -> Vec<u8> {
    // Literal property framing follows the unchanged slot-property fixture.
    let name = |text: &str| {
        let bytes = text
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        [(bytes.len() as u16).to_le_bytes().as_slice(), &bytes].concat()
    };
    let block = |kind: u16, bytes: &[u8]| {
        [
            ((bytes.len() + 6) as u32).to_le_bytes().as_slice(),
            &kind.to_le_bytes(),
            bytes,
        ]
        .concat()
    };
    let property = [
        ((value.len() + 8) as u16).to_le_bytes().as_slice(),
        &[0, native_type, 0, 0],
        &(value.len() as u16).to_le_bytes(),
        value,
    ]
    .concat();
    let label = name("Score");
    let group = [
        ((label.len() + 4) as u32).to_le_bytes().as_slice(),
        &label,
        &property,
    ]
    .concat();
    let properties = [
        b"MR2\0".as_slice(),
        &block(0x80, &name(key)),
        &block(1, &group),
    ]
    .concat();
    let mut item = support::record(10, 0, 1, 0, b"\xff\xfeItems");
    item.truncate(item.len() - 7);
    support::put16(&mut item, 0, 6);
    let start = item.len() as u16;
    item.extend((0x8000_0000 | properties.len() as u32).to_le_bytes());
    item.extend([0; 8]);
    item.extend(properties);
    item.extend((item.len() as u16).to_le_bytes());
    item.extend(start.to_le_bytes());
    item.extend(16_u16.to_le_bytes());
    item.extend(2_u16.to_le_bytes());
    item.push(0x3f);
    let mut bytes = super::fixture::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    let catalog = support::data_page(
        2,
        &[
            (
                0,
                support::record(2, 0, 1, 0x8000_0002, b"\xff\xfeMSysObjects"),
            ),
            (
                0,
                support::record(7, 0, 1, 0x8000_0002, b"\xff\xfeMSysRelationships"),
            ),
            (0, item),
        ],
    );
    bytes[4 * PAGE..5 * PAGE].copy_from_slice(&catalog);
    let mut definition =
        super::fixture::definition(&[("Id", 4, 0, 4), ("Score", 10, 0, 510)], 1, 11 << 8);
    definition[99..103].copy_from_slice(&[9, 4, 0, 1]);
    definition[103] = 2;
    definition[104] = 1;
    bytes[10 * PAGE..11 * PAGE].copy_from_slice(&definition);
    let record = vec![2, 0, 1, 0, 0, 0, b'A', 0, 8, 0, 6, 0, 1, 0, 3];
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&support::data_page(10, &[(0, record)]));
    bytes
}

fn hex(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2));
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn keys(family: Family, descending: bool) -> (Vec<u8>, Vec<u8>) {
    let pair = match (family, descending) {
        (Family::Legacy, false) => ("7f4a4a4a0100", "7f7878780100"),
        (Family::Legacy, true) => ("80b5b5b5feff00", "80878787feff00"),
        (Family::Modern, false) => ("7f0e020e020e020100", "7f0ea90ea90ea90100"),
        (Family::Modern, true) => ("80f1fdf1fdf1fdfeff00", "80f156f156f156feff00"),
        (Family::Binary, false) => ("7f000000000000000001", "7fff0000000000000001"),
        (Family::Binary, true) => ("80fffffffffffffffffe", "8000fffffffffffffffe"),
    };
    (hex(pair.0), hex(pair.1))
}

fn record(row: usize, binary: bool) -> Vec<u8> {
    let mut result = 2_u16.to_le_bytes().to_vec();
    result.extend((row as i32 + 1).to_le_bytes());
    result.extend(if binary {
        vec![0]
    } else {
        b"a\0a\0a\0".to_vec()
    });
    result.extend((result.len() as u16).to_le_bytes());
    result.extend(6_u16.to_le_bytes());
    result.extend(1_u16.to_le_bytes());
    result.push(3);
    result
}

fn reference(row: usize) -> u32 {
    ((DATA[row / PER_PAGE] as u32) << 8) | (row % PER_PAGE) as u32
}

fn wire(key: &[u8], row: usize) -> Vec<u8> {
    [key, &reference(row).to_be_bytes()].concat()
}

fn common(entries: &[Vec<u8>], node: bool) -> usize {
    let ignored = if node { 4 } else { 0 };
    let mut shared = entries[0].len() - ignored;
    for entry in &entries[1..] {
        shared = shared.min(entry.len() - ignored);
        shared = entries[0][..shared]
            .iter()
            .zip(&entry[..shared])
            .take_while(|(left, right)| left == right)
            .count();
    }
    shared
}

fn payload(entries: &[Vec<u8>], seed: usize, node: bool) -> (usize, usize) {
    let best = common(entries, node);
    let total: usize = entries.iter().map(Vec::len).sum();
    let mut prefix = seed.min(best);
    if total - prefix * (entries.len() - 1) > CAPACITY {
        prefix = best;
    }
    let length = total - prefix * (entries.len() - 1);
    assert!(length <= CAPACITY);
    (prefix, length)
}

fn page(
    original: Option<&[u8]>,
    entries: &[Vec<u8>],
    node: bool,
    seed: usize,
    previous: usize,
    next: usize,
    tail: usize,
) -> Vec<u8> {
    let mut result = original.map_or_else(|| vec![0; PAGE], <[u8]>::to_vec);
    result[..2].copy_from_slice(&[if node { 3 } else { 4 }, 1]);
    result[4..8].copy_from_slice(&10_u32.to_le_bytes());
    result[12..16].copy_from_slice(&(previous as u32).to_le_bytes());
    result[16..20].copy_from_slice(&(next as u32).to_le_bytes());
    result[20..24].copy_from_slice(&(tail as u32).to_le_bytes());
    result[26] = u8::from(node);
    result[27..ENTRY_START].fill(0);
    let (prefix, length) = payload(entries, seed, node);
    result[24..26].copy_from_slice(&(prefix as u16).to_le_bytes());
    result[2..4].copy_from_slice(&((CAPACITY - length) as u16).to_le_bytes());
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        let shared = if position == 0 { 0 } else { prefix };
        let suffix = &entry[shared..];
        result[ENTRY_START + end..ENTRY_START + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        result[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, length);
    assert_eq!(decode(&result), entries);
    result
}

fn decode(page: &[u8]) -> Vec<Vec<u8>> {
    let prefix = u16::from_le_bytes(page[24..26].try_into().unwrap()) as usize;
    let mut entries: Vec<Vec<u8>> = Vec::new();
    let mut previous = 0;
    for end in 1..=CAPACITY {
        if page[27 + end / 8] & (1 << (end % 8)) == 0 {
            continue;
        }
        let mut entry = if entries.is_empty() {
            Vec::new()
        } else {
            entries[0][..prefix].to_vec()
        };
        entry.extend(&page[ENTRY_START + previous..ENTRY_START + end]);
        entries.push(entry);
        previous = end;
    }
    let free = u16::from_le_bytes(page[2..4].try_into().unwrap()) as usize;
    assert_eq!(previous + free, CAPACITY);
    entries
}

pub fn source(family: Family, descending: bool, alias: bool) -> Model {
    let mut seed = match family {
        Family::Legacy => encoded_model::text(0, descending, alias),
        Family::Modern => encoded_model::text(1, descending, alias),
        Family::Binary => encoded_model::binary(descending, alias),
    };
    seed.bytes.resize(EOF * PAGE, 0);
    let bytes = &mut seed.bytes;
    // The copied real variable-field metadata keeps General descriptors and
    // native255-byte Binary /510-byte Text declarations. This write profile is
    // ordinary flag2, uncompressed Text, ext0; old DELETE flag0 is not authority.
    bytes[10 * PAGE + 115] = 2;
    bytes[10 * PAGE + 116] = 0;
    support::put32(bytes, 10 * PAGE + 16, ROWS as u32);
    support::put32(bytes, 10 * PAGE + 67, ROWS as u32);
    let binary = matches!(family, Family::Binary);
    let records = (0..ROWS).map(|row| record(row, binary)).collect::<Vec<_>>();
    for (block, &number) in DATA.iter().enumerate() {
        let rows = records[block * PER_PAGE..(block + 1) * PER_PAGE]
            .iter()
            .cloned()
            .map(|record| (0, record))
            .collect::<Vec<_>>();
        bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(&support::data_page(10, &rows));
    }
    let mut owned = vec![0; 69];
    for number in DATA {
        owned[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[11 * PAGE..12 * PAGE].copy_from_slice(&support::data_page(0, &[(0, owned)]));
    let mut index_owned = vec![0; 69];
    index_owned[5 + ROOT / 8] |= 1 << (ROOT % 8);
    bytes[14 * PAGE..15 * PAGE].copy_from_slice(&support::data_page(0, &[(0, index_owned)]));
    let mut global = vec![0; 69];
    for number in EOF..512 {
        global[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[PAGE..2 * PAGE].copy_from_slice(&support::data_page(1, &[(0, global)]));
    let (old_key, new_key) = keys(family, descending);
    let old_entries = (0..ROWS).map(|row| wire(&old_key, row)).collect::<Vec<_>>();
    let mut original = vec![0xa7; PAGE];
    original[..ENTRY_START].fill(0);
    let mut old_root = page(
        Some(&original),
        &old_entries,
        false,
        common(&old_entries, false),
        0,
        0,
        0,
    );
    old_root[8..12].fill(0);
    bytes[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&old_root);
    let mut expected = bytes.clone();
    expected.resize((EOF + 3) * PAGE, 0);
    let row_start =
        u16::from_le_bytes(bytes[12 * PAGE + 14..12 * PAGE + 16].try_into().unwrap()) as usize;
    let replacement = if binary {
        vec![255]
    } else {
        b"z\0z\0z\0".to_vec()
    };
    let offset = 12 * PAGE + row_start + 6;
    expected[offset..offset + replacement.len()].copy_from_slice(&replacement);
    let mut order = (1..ROWS).collect::<Vec<_>>();
    if descending {
        order.insert(0, 0);
    } else {
        order.push(0);
    }
    let entries = order
        .iter()
        .map(|&row| wire(if row == 0 { &new_key } else { &old_key }, row))
        .collect::<Vec<_>>();
    let (leaf_order, leaf_counts) = if descending {
        ([EOF + 2, EOF + 1, EOF], [225, 225, 450])
    } else {
        ([EOF + 1, EOF + 2, EOF], [450, 225, 225])
    };
    let mut position = 0;
    let mut bounds = Vec::new();
    let mut decoded = Vec::new();
    for (index, (&number, &count)) in leaf_order.iter().zip(&leaf_counts).enumerate() {
        let subset = &entries[position..position + count];
        let leaf = page(
            None,
            subset,
            false,
            0,
            if index == 0 { 0 } else { leaf_order[index - 1] },
            leaf_order.get(index + 1).copied().unwrap_or(0),
            0,
        );
        decoded.extend(decode(&leaf));
        if index < 2 {
            let mut maximum = subset.last().unwrap().clone();
            maximum.extend((number as u32).to_be_bytes());
            bounds.push(maximum);
        }
        expected[number * PAGE..(number + 1) * PAGE].copy_from_slice(&leaf);
        position += count;
    }
    assert_eq!(decoded, entries);
    let root = page(Some(&old_root), &bounds, true, 0, 0, 0, EOF);
    expected[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&root);
    // Exact existing inline map cells: data_page places the69-byte row at4027.
    for number in EOF..EOF + 3 {
        expected[14 * PAGE + 4027 + 5 + number / 8] |= 1 << (number % 8);
        expected[PAGE + 4027 + 5 + number / 8] &= !(1 << (number % 8));
    }
    Model {
        bytes: seed.bytes,
        expected,
        records,
        old_key,
        new_key,
        leaf_order,
        leaf_counts,
    }
}

fn sort_entries(entries: &mut [Vec<u8>]) {
    entries.sort_by(|left, right| {
        let left_key = left.len() - 4;
        let right_key = right.len() - 4;
        left[..left_key]
            .cmp(&right[..right_key])
            .then_with(|| left[left_key..].cmp(&right[right_key..]))
    });
}

pub fn promotion_case(family: Family) -> Case {
    let base = root_case(family, false);
    let mut bytes = base.bytes;
    bytes.resize(600 * PAGE, 0);
    // Complete literal declaration lists: the system catalog has only inline
    // properties; neither remaining table has live or historical LVAL owners.
    support::put16(&mut bytes, 2 * PAGE + 398, 0xffff);
    let relationship_names = [
        "szRelationship",
        "grbit",
        "ccolumn",
        "icolumn",
        "szReferencedObject",
        "szObject",
        "szReferencedColumn",
        "szColumn",
    ];
    let relationship_end = 63
        + 8 * 25
        + relationship_names
            .iter()
            .map(|name| 2 + name.encode_utf16().count() * 2)
            .sum::<usize>();
    support::put16(&mut bytes, 7 * PAGE + relationship_end, 0xffff);
    support::put16(&mut bytes, 10 * PAGE + 291, 0xffff);
    let mut global = vec![0; 69];
    global[0] = 1;
    global[1..5].copy_from_slice(&599_u32.to_le_bytes());
    bytes[PAGE..2 * PAGE].copy_from_slice(&support::data_page(1, &[(0, global)]));
    let mut bitmap = vec![0; PAGE];
    bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
    for number in 600..32736 {
        bitmap[4 + number / 8] |= 1 << (number % 8);
    }
    bytes[599 * PAGE..600 * PAGE].copy_from_slice(&bitmap);
    let mut expected = bytes.clone();
    expected.resize(604 * PAGE, 0);
    let start =
        u16::from_le_bytes(bytes[12 * PAGE + 14..12 * PAGE + 16].try_into().unwrap()) as usize;
    let replacement = if matches!(family, Family::Binary) {
        vec![255]
    } else {
        b"z\0z\0z\0".to_vec()
    };
    expected[12 * PAGE + start + 6..12 * PAGE + start + 6 + replacement.len()]
        .copy_from_slice(&replacement);
    let (old, new) = keys(family, false);
    let entries = (1..ROWS)
        .map(|row| wire(&old, row))
        .chain(std::iter::once(wire(&new, 0)))
        .collect::<Vec<_>>();
    let pages = [602, 603, 600];
    let counts = [450, 225, 225];
    let mut position = 0;
    let mut bounds = Vec::new();
    for (ordinal, (&number, &count)) in pages.iter().zip(&counts).enumerate() {
        let subset = &entries[position..position + count];
        let leaf = page(
            None,
            subset,
            false,
            0,
            if ordinal == 0 { 0 } else { pages[ordinal - 1] },
            pages.get(ordinal + 1).copied().unwrap_or(0),
            0,
        );
        expected[number * PAGE..(number + 1) * PAGE].copy_from_slice(&leaf);
        if ordinal < 2 {
            let mut bound = subset.last().unwrap().clone();
            bound.extend((number as u32).to_be_bytes());
            bounds.push(bound);
        }
        position += count;
    }
    let node = page(
        Some(&bytes[ROOT * PAGE..(ROOT + 1) * PAGE]),
        &bounds,
        true,
        0,
        0,
        0,
        600,
    );
    expected[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&node);
    let row = &mut expected[14 * PAGE + 4027..15 * PAGE];
    row.fill(0);
    row[0] = 1;
    row[1..5].copy_from_slice(&601_u32.to_le_bytes());
    let mut owned = vec![0; PAGE];
    owned[..4].copy_from_slice(&[5, 1, 0, 0]);
    for number in [ROOT, 600, 602, 603] {
        owned[4 + number / 8] |= 1 << (number % 8);
    }
    expected[601 * PAGE..602 * PAGE].copy_from_slice(&owned);
    for number in 600..604 {
        expected[599 * PAGE + 4 + number / 8] &= !(1 << (number % 8));
    }
    Case {
        bytes,
        expected,
        row: 0,
        old: base.old,
        new: base.new,
    }
}
