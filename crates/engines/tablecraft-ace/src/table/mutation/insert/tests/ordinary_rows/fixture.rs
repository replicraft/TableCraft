//! Genuine ordinary schema/records with the indexed Int32 after a companion.
use super::super::model as base;
use super::model::Row;
use crate::test_support::base as catalog;
use tablecraft_testkit::access as raw;

pub(super) const PAGE: usize = 4096;

#[derive(Clone, Copy)]
pub(in crate::table::mutation::insert) enum Index {
    Primary,
    Alias,
    Ordinary,
    Descending,
    Composite,
    DoublePrimary,
    None,
}

pub(in crate::table::mutation::insert) struct Case {
    pub before: Vec<u8>,
    pub rows: Vec<Row>,
    pub variables: bool,
    pub index: Index,
    pub physical: usize,
    pub logical: usize,
}

fn put(bytes: &mut [u8], offset: usize, value: u32, width: usize) {
    bytes[offset..offset + width].copy_from_slice(&value.to_le_bytes()[..width]);
}

fn install(bytes: &mut [u8], page: usize, data: &[u8]) {
    bytes[page * PAGE..(page + 1) * PAGE].copy_from_slice(data);
}

fn inline(members: impl IntoIterator<Item = usize>) -> Vec<u8> {
    let mut row = vec![0; 69];
    for number in members {
        row[5 + number / 8] |= 1 << (number % 8);
    }
    row
}

fn component(id: i32, descending: bool) -> Vec<u8> {
    let mut bytes = vec![if descending { 128 } else { 127 }];
    let mut value = ((id as u32) ^ 0x80000000).to_be_bytes();
    if descending {
        value.iter_mut().for_each(|byte| *byte = !*byte);
    }
    bytes.extend(value);
    bytes
}

fn leaf(original: Option<&[u8]>, keys: &[(i32, i32, u32)], index: Index) -> Vec<u8> {
    let mut wires = keys
        .iter()
        .map(|(id, earlier, reference)| {
            let mut wire = component(*id, matches!(index, Index::Descending));
            if matches!(index, Index::Composite) {
                wire.extend(component(*earlier, false));
            }
            wire.extend(reference.to_be_bytes());
            wire
        })
        .collect::<Vec<_>>();
    wires.sort_unstable();
    let mut page = original.map_or_else(|| vec![0xa7; PAGE], <[u8]>::to_vec);
    page[..480].fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    put(&mut page, 4, 10, 4);
    let mut end = 0;
    for wire in wires {
        page[480 + end..480 + end + wire.len()].copy_from_slice(&wire);
        end += wire.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    put(&mut page, 2, (3616 - end) as u32, 2);
    page
}

pub(in crate::table::mutation::insert) fn source(
    code: u8,
    variables: bool,
    full: bool,
    index: Index,
    default: bool,
) -> Case {
    let mut before = base::source(
        code,
        matches!(index, Index::Alias | Index::DoublePrimary),
        false,
        default.then_some(("DefaultValue", "1")),
    );
    if !default {
        let plain = catalog::database();
        before[4 * PAGE..5 * PAGE].copy_from_slice(&plain[4 * PAGE..5 * PAGE]);
    }
    let mut fields = vec![
        ("Earlier", 4, 0, 4, None),
        ("Flag", 1, 0, 0, None),
        ("ID", 4, 8, 4, None),
        ("Tiny", 2, 4, 1, None),
        ("Small", 3, 6, 2, None),
        ("Large", 19, 12, 8, None),
        ("Money", 5, 20, 8, None),
        ("Ratio", 6, 28, 4, None),
        ("Measure", 7, 32, 8, None),
        ("When", 8, 40, 8, None),
        ("Token", 15, 48, 16, None),
    ];
    if variables {
        fields.extend([
            ("LaterText", 10, 0, 510, Some(2)),
            ("FirstBytes", 9, 0, 255, Some(0)),
            ("Score", 10, 0, 510, Some(1)),
            ("LastBytes", 9, 0, 510, Some(3)),
        ]);
    }
    fields.extend([("Spare", 4, 64, 4, None), ("LastFlag", 1, 0, 0, None)]);
    let has_index = !matches!(index, Index::None);
    let aliases = matches!(index, Index::Alias | Index::DoublePrimary);
    let count = fields.len();
    let mut definition = vec![0; PAGE];
    definition[..2].copy_from_slice(&[2, 1]);
    put(&mut definition, 8, PAGE as u32, 4);
    put(&mut definition, 16, 3, 4);
    put(&mut definition, 41, count as u32, 2);
    put(&mut definition, 43, if variables { 4 } else { 0 }, 2);
    put(&mut definition, 45, count as u32, 2);
    put(
        &mut definition,
        47,
        if !has_index {
            0
        } else if aliases {
            2
        } else {
            1
        },
        4,
    );
    put(&mut definition, 51, u32::from(has_index), 4);
    put(&mut definition, 55, 11 << 8, 4);
    put(&mut definition, 59, (11 << 8) | 1, 4);
    if has_index {
        put(&mut definition, 67, 17, 4);
    }
    let column_start = 63 + if has_index { 12 } else { 0 };
    let mut names = column_start + count * 25;
    for (number, (name, kind, offset, length, variable)) in fields.into_iter().enumerate() {
        let start = column_start + number * 25;
        definition[start] = kind;
        put(&mut definition, start + 5, number as u32, 2);
        definition[start + 15] = if variable.is_some() { 2 } else { 3 };
        if let Some(variable) = variable {
            put(&mut definition, start + 7, variable, 2);
        }
        if number == 11 && variables {
            definition[start + 16] = 1;
        }
        put(&mut definition, start + 21, offset, 2);
        put(&mut definition, start + 23, length, 2);
        let name = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        put(&mut definition, names, name.len() as u32, 2);
        names += 2;
        definition[names..names + name.len()].copy_from_slice(&name);
        names += name.len();
    }
    let physical = names;
    let logical = physical + if has_index { 52 } else { 0 };
    if has_index {
        for slot in 0..10 {
            put(&mut definition, physical + 4 + slot * 3, 0xffff, 2);
        }
        put(&mut definition, physical + 4, 2, 2);
        definition[physical + 6] = u8::from(!matches!(index, Index::Descending));
        if matches!(index, Index::Composite) {
            put(&mut definition, physical + 7, 0, 2);
            definition[physical + 9] = 1;
        }
        put(&mut definition, physical + 34, 14 << 8, 4);
        put(&mut definition, physical + 38, 13, 4);
        definition[physical + 46] = 0x89;
        definition[logical + 23] = u8::from(!matches!(index, Index::Ordinary));
        if aliases {
            put(&mut definition, logical + 28 + 4, 1, 4);
            definition[logical + 28 + 23] = u8::from(matches!(index, Index::DoublePrimary));
        }
    }
    names = logical
        + if !has_index {
            0
        } else if aliases {
            56
        } else {
            28
        };
    for name in if !has_index {
        vec![]
    } else if aliases {
        vec!["PrimaryID", "PrimaryAlias"]
    } else {
        vec!["PrimaryID"]
    } {
        let name = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        put(&mut definition, names, name.len() as u32, 2);
        names += 2;
        definition[names..names + name.len()].copy_from_slice(&name);
        names += name.len();
    }
    put(&mut definition, names, 0xffff, 2);
    install(&mut before, 10, &definition);
    let rows = vec![
        Row::rich(1),
        Row::rich(2),
        if matches!(index, Index::Composite) {
            Row::rich(3)
        } else {
            Row::nulls(3)
        },
    ];
    let mut encoded = rows
        .iter()
        .map(|row| (0, row.record(variables)))
        .collect::<Vec<_>>();
    if full {
        let occupied = encoded.iter().map(|(_, row)| row.len()).sum::<usize>();
        let padding = 4082 - 2 * 4 - occupied - 32;
        encoded.push((0x8000, vec![0x9d; padding]));
    }
    let free = 4082 - 2 * encoded.len() - encoded.iter().map(|(_, row)| row.len()).sum::<usize>();
    let mut data = raw::data_page(10, &encoded);
    put(&mut data, 2, free as u32, 2);
    install(&mut before, 12, &data);
    install(
        &mut before,
        11,
        &raw::data_page(0, &[(0, inline([12])), (0, inline([12]))]),
    );
    install(
        &mut before,
        14,
        &raw::data_page(
            0,
            &[(0, inline(if has_index { vec![13] } else { Vec::new() }))],
        ),
    );
    install(&mut before, 1, &raw::data_page(1, &[(0, inline(16..512))]));
    let keys = rows
        .iter()
        .enumerate()
        .map(|(slot, row)| (row.id, row.earlier.unwrap_or(0), (12 << 8) | slot as u32))
        .collect::<Vec<_>>();
    install(&mut before, 13, &leaf(None, &keys, index));
    let mut case = Case {
        before,
        rows,
        variables,
        index,
        physical,
        logical,
    };
    if variables && !default {
        empty_rules(&mut case, &[("LaterText", true), ("Score", true)]);
    }
    case
}

pub(in crate::table::mutation::insert) fn expected(case: &Case, rows: &[Row]) -> Vec<u8> {
    let mut bytes = case.before.clone();
    let mut hints = vec![12_usize];
    let mut owned = vec![12_usize];
    let mut keys = case
        .rows
        .iter()
        .enumerate()
        .map(|(slot, row)| (row.id, row.earlier.unwrap_or(0), (12 << 8) | slot as u32))
        .collect::<Vec<_>>();
    for row in rows {
        let encoded = row.record(case.variables);
        let mut number = *hints.last().unwrap();
        let mut base = number * PAGE;
        let mut count = usize::from(u16::from_le_bytes(
            bytes[base + 12..base + 14].try_into().unwrap(),
        ));
        let end = if count == 0 {
            PAGE
        } else {
            usize::from(u16::from_le_bytes(
                bytes[base + 12 + count * 2..base + 14 + count * 2]
                    .try_into()
                    .unwrap(),
            )) & 0x1fff
        };
        let mut stop = end;
        if count == 255 || encoded.len() + 2 > end - (14 + count * 2) {
            hints.pop();
            number = bytes.len() / PAGE;
            bytes.extend(vec![0; PAGE]);
            base = number * PAGE;
            bytes[base..base + 2].copy_from_slice(&[1, 1]);
            put(&mut bytes, base + 4, 10, 4);
            count = 0;
            stop = PAGE;
            hints.push(number);
            owned.push(number);
        }
        let start = stop - encoded.len();
        bytes[base + start..base + stop].copy_from_slice(&encoded);
        put(&mut bytes, base + 14 + 2 * count, start as u32, 2);
        put(&mut bytes, base + 12, (count + 1) as u32, 2);
        put(&mut bytes, base + 2, (start - 16 - 2 * count) as u32, 2);
        keys.push((
            row.id,
            row.earlier.unwrap_or(0),
            (number as u32) << 8 | count as u32,
        ));
    }
    install(
        &mut bytes,
        13,
        &leaf(Some(&case.before[13 * PAGE..14 * PAGE]), &keys, case.index),
    );
    put(
        &mut bytes,
        10 * PAGE + 16,
        (case.rows.len() + rows.len()) as u32,
        4,
    );
    put(&mut bytes, 10 * PAGE + 67, 17 + rows.len() as u32, 4);
    install(
        &mut bytes,
        11,
        &raw::data_page(0, &[(0, inline(owned)), (0, inline(hints))]),
    );
    let eof = bytes.len() / PAGE;
    install(&mut bytes, 1, &raw::data_page(1, &[(0, inline(eof..512))]));
    bytes
}

pub(super) fn empty_rules(case: &mut Case, columns: &[(&str, bool)]) {
    let declarations = columns
        .iter()
        .map(|(name, value)| (*name, 1_u8, vec![if *value { 0xff } else { 0 }]))
        .collect::<Vec<_>>();
    property_groups(case, &declarations);
}

pub(super) fn remove_empty_rules(case: &mut Case) {
    let original = catalog::database();
    case.before[4 * PAGE..5 * PAGE].copy_from_slice(&original[4 * PAGE..5 * PAGE]);
}

pub(super) fn wrongly_typed_empty_rule(case: &mut Case) {
    let text = "true".encode_utf16().flat_map(u16::to_le_bytes).collect();
    property_groups(case, &[("Score", 10, text)]);
}

fn property_groups(case: &mut Case, declarations: &[(&str, u8, Vec<u8>)]) {
    fn name(value: &str) -> Vec<u8> {
        let bytes = value
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        [(bytes.len() as u16).to_le_bytes().as_slice(), &bytes].concat()
    }
    fn block(kind: u16, body: &[u8]) -> Vec<u8> {
        [
            ((body.len() + 6) as u32).to_le_bytes().as_slice(),
            &kind.to_le_bytes(),
            body,
        ]
        .concat()
    }
    let mut properties = b"MR2\0".to_vec();
    properties.extend(block(0x80, &name("AllowZeroLength")));
    for (column, kind, data) in declarations {
        let property = [
            ((data.len() + 8) as u16).to_le_bytes().as_slice(),
            &[0, *kind, 0, 0],
            &(data.len() as u16).to_le_bytes(),
            data,
        ]
        .concat();
        let label = name(column);
        let group = [
            ((label.len() + 4) as u32).to_le_bytes().as_slice(),
            &label,
            &property,
        ]
        .concat();
        properties.extend(block(1, &group));
    }
    let mut item = raw::record(10, 0, 1, 0, b"\xff\xfeItems");
    item.truncate(item.len() - 7);
    put(&mut item, 0, 6, 2);
    let start = item.len() as u16;
    item.extend((0x80000000_u32 | properties.len() as u32).to_le_bytes());
    item.extend([0; 8]);
    item.extend(properties);
    item.extend((item.len() as u16).to_le_bytes());
    item.extend(start.to_le_bytes());
    item.extend([16, 0, 2, 0, 0x3f]);
    let catalog = raw::data_page(
        2,
        &[
            (0, raw::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects")),
            (
                0,
                raw::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
            ),
            (0, item),
        ],
    );
    install(&mut case.before, 4, &catalog);
}
