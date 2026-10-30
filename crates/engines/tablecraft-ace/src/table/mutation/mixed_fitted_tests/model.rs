//! Literal rows, key weights and complete images; no mutation encoder is called.
use super::base;
use tablecraft_core::values::Value;
use tablecraft_testkit::access as raw;

pub const PAGE: usize = 4096;
const ROOTS: [usize; 3] = [13, 15, 16];

#[derive(Clone, Copy, Debug)]
pub enum Family {
    Legacy,
    Modern,
    Binary,
}

#[derive(Clone, Copy, Debug)]
pub enum Recipe {
    Permutation,
    Shorter,
    Nulls,
}

#[derive(Clone)]
pub struct Row {
    pub id: i32,
    pub age: Option<i32>,
    pub name: Option<Vec<u8>>,
}

pub struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub rows: Vec<Row>,
    pub final_rows: Vec<Row>,
    pub records: Vec<Vec<u8>>,
    pub final_records: Vec<Vec<u8>>,
    pub changed: Vec<usize>,
    pub family: Family,
    pub forwarded: bool,
}

impl Family {
    fn text(self) -> bool {
        !matches!(self, Self::Binary)
    }

    fn low(self) -> Vec<u8> {
        if self.text() { vec![b'a'] } else { vec![0] }
    }

    fn high(self) -> Vec<u8> {
        if self.text() { vec![0xe9] } else { vec![255] }
    }

    fn long(self) -> Vec<u8> {
        if self.text() {
            vec![b'a', b'a']
        } else {
            vec![0, 255]
        }
    }
}

pub fn values(row: &Row, family: Family) -> Vec<Value> {
    vec![
        Value::Int32(row.id),
        row.age.map_or(Value::Null, Value::Int32),
        name_value(row, family),
        Value::Text(format!("Z{}", row.id)),
    ]
}

pub fn name_value(row: &Row, family: Family) -> Value {
    match &row.name {
        None => Value::Null,
        Some(bytes) if family.text() => {
            Value::Text(bytes.iter().map(|&byte| char::from(byte)).collect())
        }
        Some(bytes) => Value::Binary(bytes.clone()),
    }
}

fn payload(name: &[u8], family: Family, compressed: bool) -> Vec<u8> {
    if !family.text() {
        return name.to_vec();
    }
    if compressed {
        [vec![0xff, 0xfe], name.to_vec()].concat()
    } else {
        name.iter()
            .flat_map(|&byte| u16::from(byte).to_le_bytes())
            .collect()
    }
}

fn record(row: &Row, family: Family, compressed: bool) -> Vec<u8> {
    let mut bytes = 4_u16.to_le_bytes().to_vec();
    bytes.extend(row.id.to_le_bytes());
    bytes.extend(row.age.unwrap_or(0x6655_4433).to_le_bytes());
    let name = row.name.as_ref().map_or_else(
        || vec![0xd3, 0, 0xff, 0x91],
        |name| payload(name, family, compressed),
    );
    bytes.extend(name);
    let name_end = bytes.len() as u16;
    bytes.extend([b'Z', 0, b'0' + row.id as u8, 0]);
    let payload_end = bytes.len() as u16;
    // Genuine capacity outside the logical variable spans, not padded text.
    bytes.extend([0x9d, 0xe1, 0xa7, 0, 0x8f, 0xb2, 0x43, 0xc9]);
    for offset in [payload_end, name_end, 10] {
        bytes.extend(offset.to_le_bytes());
    }
    bytes.extend(2_u16.to_le_bytes());
    bytes.push(9 | if row.age.is_some() { 2 } else { 0 } | if row.name.is_some() { 4 } else { 0 });
    bytes
}

fn final_record(before: &[u8], old: &Row, new: &Row, family: Family, compressed: bool) -> Vec<u8> {
    let mut after = before.to_vec();
    let footer = before.len() - 9;
    let old_name_end = usize::from(u16::from_le_bytes(
        before[footer + 2..footer + 4].try_into().unwrap(),
    ));
    let old_payload_end = usize::from(u16::from_le_bytes(
        before[footer..footer + 2].try_into().unwrap(),
    ));
    let name = if old.name == new.name {
        before[10..old_name_end].to_vec()
    } else {
        new.name
            .as_ref()
            .map_or_else(Vec::new, |name| payload(name, family, compressed))
    };
    let name_end = 10 + name.len();
    let payload_end = name_end + old_payload_end - old_name_end;
    assert!(
        payload_end <= footer,
        "literal replacement must fit original capacity"
    );
    after[10..name_end].copy_from_slice(&name);
    after[name_end..payload_end].copy_from_slice(&before[old_name_end..old_payload_end]);
    for (position, offset) in [payload_end, name_end, 10].into_iter().enumerate() {
        after[footer + position * 2..footer + position * 2 + 2]
            .copy_from_slice(&(offset as u16).to_le_bytes());
    }
    if let Some(age) = new.age {
        after[6..10].copy_from_slice(&age.to_le_bytes());
    }
    let last = after.len() - 1;
    after[last] = (before[last] & !6)
        | if new.age.is_some() { 2 } else { 0 }
        | if new.name.is_some() { 4 } else { 0 };
    after
}

fn integer(value: Option<i32>, descending: bool) -> Vec<u8> {
    let Some(value) = value else {
        return vec![if descending { 255 } else { 0 }];
    };
    let mut bytes = vec![if descending { 128 } else { 127 }];
    bytes.extend(
        ((value as u32) ^ 0x80000000)
            .to_be_bytes()
            .map(|byte| if descending { !byte } else { byte }),
    );
    bytes
}

fn name_key(value: Option<&[u8]>, family: Family, descending: bool) -> Vec<u8> {
    let Some(value) = value else {
        return vec![if descending { 255 } else { 0 }];
    };
    let mut bytes = vec![if descending { 128 } else { 127 }];
    if matches!(family, Family::Binary) {
        for chunk in value.chunks(8) {
            let mut segment = [if descending { 255 } else { 0 }; 9];
            for (to, &from) in segment.iter_mut().zip(chunk) {
                *to = if descending { !from } else { from };
            }
            segment[8] = if descending {
                !(chunk.len() as u8)
            } else {
                chunk.len() as u8
            };
            bytes.extend(segment);
        }
        return bytes;
    }
    // Short immutable weights already qualified by the repository's independent
    // General legacy/modern encoded-key corpus. No generated table is imported.
    let mut weights = match (family, value) {
        (Family::Legacy, b"") | (Family::Modern, b"") => vec![1, 0],
        (Family::Legacy, b"a") => vec![0x4a, 1, 0],
        (Family::Legacy, b"aa") => vec![0x4a, 0x4a, 1, 0],
        (Family::Legacy, [0xe9]) => vec![0x51, 1, 0x0e, 0],
        (Family::Modern, b"a") => vec![0x0e, 2, 1, 0],
        (Family::Modern, b"aa") => vec![0x0e, 2, 0x0e, 2, 1, 0],
        (Family::Modern, [0xe9]) => vec![0x0e, 0x21, 1, 0x0e, 0],
        _ => panic!("closed literal name family"),
    };
    if descending {
        weights.iter_mut().for_each(|byte| *byte = !*byte);
        weights.push(0);
    }
    bytes.extend(weights);
    bytes
}

pub(super) fn index_page(
    original: &[u8],
    rows: &[Row],
    physical: usize,
    family: Family,
    descending: bool,
) -> Vec<u8> {
    let mut page = original.to_vec();
    page[27..480].fill(0);
    let mut entries = rows
        .iter()
        .enumerate()
        .map(|(slot, row)| {
            let mut key = match physical {
                0 => integer(row.age, !descending),
                1 => name_key(row.name.as_deref(), family, descending),
                2 => {
                    let mut key = name_key(row.name.as_deref(), family, descending);
                    key.extend(integer(row.age, !descending));
                    key
                }
                _ => unreachable!(),
            };
            key.extend(((12_u32 << 8) | slot as u32).to_be_bytes());
            key
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left[..left.len() - 4]
            .cmp(&right[..right.len() - 4])
            .then(left[left.len() - 4..].cmp(&right[right.len() - 4..]))
    });
    let mut end = 0;
    for entry in entries {
        page[480 + end..480 + end + entry.len()].copy_from_slice(&entry);
        end += entry.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    raw::put16(&mut page, 2, (3616 - end) as u16);
    raw::put16(&mut page, 24, 0);
    page
}

fn data_page(owner: u32, rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut page = raw::data_page(owner, rows);
    let first = PAGE - rows.iter().map(|(_, row)| row.len()).sum::<usize>();
    raw::put16(&mut page, 2, (first - 14 - rows.len() * 2) as u16);
    page
}

pub fn case(code: u8, family: Family, descending: bool, recipe: Recipe, forwarded: bool) -> Case {
    let compressed = family.text() && code == 5;
    let low = family.low();
    let high = family.high();
    let mut rows = vec![
        Row {
            id: 1,
            age: Some(10),
            name: Some(low.clone()),
        },
        Row {
            id: 2,
            age: Some(20),
            name: Some(high.clone()),
        },
        Row {
            id: 3,
            age: Some(20),
            name: Some(low.clone()),
        },
        Row {
            id: 4,
            age: Some(10),
            name: Some(high.clone()),
        },
    ];
    if matches!(recipe, Recipe::Shorter) {
        rows[0].name = Some(family.long());
    }
    if matches!(recipe, Recipe::Nulls) {
        rows[2] = Row {
            id: 3,
            age: Some(30),
            name: Some(family.long()),
        };
        rows[3] = Row {
            id: 4,
            age: None,
            name: None,
        };
    }
    let mut final_rows = rows.clone();
    let changed = match recipe {
        Recipe::Permutation => {
            final_rows[0].age = Some(20);
            final_rows[0].name = Some(high);
            final_rows[1].age = Some(10);
            final_rows[1].name = Some(low);
            vec![0, 1]
        }
        Recipe::Shorter => {
            final_rows[0].age = Some(11);
            final_rows[0].name = Some(low);
            vec![0]
        }
        Recipe::Nulls => {
            final_rows[0].age = None;
            final_rows[0].name = None;
            final_rows[3].age = Some(40);
            final_rows[3].name = Some(Vec::new());
            vec![0, 3]
        }
    };
    let records = rows
        .iter()
        .map(|row| record(row, family, compressed))
        .collect::<Vec<_>>();
    let final_records = rows
        .iter()
        .zip(&final_rows)
        .zip(&records)
        .map(|((old, new), record)| final_record(record, old, new, family, compressed))
        .collect::<Vec<_>>();
    let mut before = base::database();
    before.resize((if forwarded { 18 } else { 17 }) * PAGE, 0);
    before[4..20].copy_from_slice(b"Standard ACE DB\0");
    before[20] = code;
    let mut definition = base::definition(
        &[
            ("Id", 4, 0, 4),
            ("Age", 4, 4, 4),
            (
                "Name",
                if family.text() { 10 } else { 9 },
                0,
                if family.text() { 510 } else { 255 },
            ),
            ("Other", 10, 0, 32),
        ],
        4,
        11 << 8,
    );
    raw::put16(&mut definition, 43, 2);
    let name = 63 + 50;
    definition[name + 15] = 2;
    raw::put16(&mut definition, name + 7, 0);
    if family.text() {
        definition[name + 11..name + 15].copy_from_slice(&[
            9,
            4,
            0,
            u8::from(matches!(family, Family::Modern)),
        ]);
        definition[name + 16] = if compressed { 1 } else { 0 };
    }
    let other = 63 + 75;
    definition[other + 15] = 2;
    raw::put16(&mut definition, other + 7, 1);
    let names_end = 63
        + 100
        + ["Id", "Age", "Name", "Other"]
            .iter()
            .map(|name| 2 + name.len() * 2)
            .sum::<usize>();
    definition.copy_within(63..names_end, 99);
    definition[63..99].fill(0);
    for (physical, history) in [29, 31, 37].into_iter().enumerate() {
        raw::put32(&mut definition, 63 + physical * 12 + 4, history);
    }
    raw::put32(&mut definition, 47, 6);
    raw::put32(&mut definition, 51, 3);
    let physical_start = names_end + 36;
    for (physical, &root) in ROOTS.iter().enumerate() {
        let at = physical_start + physical * 52;
        for component in 0..10 {
            raw::put16(&mut definition, at + 4 + component * 3, 0xffff);
        }
        for (component, number, desc) in match physical {
            0 => vec![(0, 1, !descending)],
            1 => vec![(0, 2, descending)],
            _ => vec![(0, 2, descending), (1, 1, !descending)],
        } {
            raw::put16(&mut definition, at + 4 + component * 3, number);
            definition[at + 6 + component * 3] = u8::from(!desc);
        }
        raw::put32(&mut definition, at + 34, (14 << 8) | physical as u32);
        raw::put32(&mut definition, at + 38, root as u32);
        definition[at + 46] = if physical == 2 { 0x81 } else { 0x80 };
    }
    let logical = physical_start + 156;
    let mut names = logical + 6 * 28;
    for (index, name) in [
        "AgeKey",
        "AgeAlias",
        "NameKey",
        "NameAlias",
        "PairKey",
        "PairAlias",
    ]
    .iter()
    .enumerate()
    {
        raw::put32(&mut definition, logical + index * 28 + 4, index as u32);
        raw::put32(
            &mut definition,
            logical + index * 28 + 8,
            (index / 2) as u32,
        );
        let encoded = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        raw::put16(&mut definition, names, encoded.len() as u16);
        names += 2;
        definition[names..names + encoded.len()].copy_from_slice(&encoded);
        names += encoded.len();
    }
    before[10 * PAGE..11 * PAGE].copy_from_slice(&definition);
    let mut physical_records = records
        .iter()
        .cloned()
        .map(|record| (0, record))
        .collect::<Vec<_>>();
    if forwarded {
        physical_records[0].0 = 0x4000;
        physical_records[0].1.fill(0x5d);
        physical_records[0].1[..4].copy_from_slice(&(17_u32 << 8).to_le_bytes());
        before[17 * PAGE..18 * PAGE]
            .copy_from_slice(&data_page(10, &[(0x8000, records[0].clone())]));
    }
    before[12 * PAGE..13 * PAGE].copy_from_slice(&data_page(10, &physical_records));
    let mut owned = vec![0; 69];
    owned[5 + 12 / 8] |= 1 << (12 % 8);
    if forwarded {
        owned[5 + 17 / 8] |= 1 << (17 % 8);
    }
    before[11 * PAGE..12 * PAGE].copy_from_slice(&data_page(0, &[(0, owned)]));
    let maps = ROOTS
        .iter()
        .map(|&root| {
            let mut map = vec![0; 69];
            map[5 + root / 8] |= 1 << (root % 8);
            (0, map)
        })
        .collect::<Vec<_>>();
    before[14 * PAGE..15 * PAGE].copy_from_slice(&data_page(0, &maps));
    let mut global = vec![0; 69];
    for page in (if forwarded { 18 } else { 17 })..512 {
        global[5 + page / 8] |= 1 << (page % 8);
    }
    before[PAGE..2 * PAGE].copy_from_slice(&data_page(1, &[(0, global)]));
    for (physical, &root) in ROOTS.iter().enumerate() {
        let mut page = vec![0xa7; PAGE];
        page[..480].fill(0);
        page[..2].copy_from_slice(&[4, 1]);
        raw::put32(&mut page, 4, 10);
        before[root * PAGE..(root + 1) * PAGE]
            .copy_from_slice(&index_page(&page, &rows, physical, family, descending));
    }
    let mut expected = before.clone();
    for &row in &changed {
        let (page, slot) = if forwarded && row == 0 {
            (17, 0)
        } else {
            (12, row)
        };
        let at = page * PAGE + 14 + slot * 2;
        let start =
            usize::from(u16::from_le_bytes(before[at..at + 2].try_into().unwrap()) & 0x1fff);
        assert_eq!(records[row].len(), final_records[row].len());
        expected[page * PAGE + start..page * PAGE + start + final_records[row].len()]
            .copy_from_slice(&final_records[row]);
    }
    for (physical, &root) in ROOTS.iter().enumerate() {
        expected[root * PAGE..(root + 1) * PAGE].copy_from_slice(&index_page(
            &before[root * PAGE..(root + 1) * PAGE],
            &final_rows,
            physical,
            family,
            descending,
        ));
    }
    Case {
        before,
        expected,
        rows,
        final_rows,
        records,
        final_records,
        changed,
        family,
        forwarded,
    }
}
