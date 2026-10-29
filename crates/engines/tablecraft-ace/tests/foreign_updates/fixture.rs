use super::{base, support};

// Repository-authored self-relationship bytes, not an Access fixture. The local
// primary and reciprocal parent share one tree; the child has an ordinary alias.
pub fn database(code: u8, descending: bool, cascade: bool) -> Vec<u8> {
    let mut bytes = base::with_rule("Caption", "foreign updates");
    bytes.resize(17 * 4096, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    let definition = &mut bytes[10 * 4096..11 * 4096];
    definition.copy_within(63..131, 87);
    definition[63..87].fill(0);
    support::put32(definition, 47, 4);
    support::put32(definition, 51, 2);
    for (position, (column, root, usage, flags)) in [(0, 13, 14, 0x81), (1, 16, 15, 0x80)]
        .into_iter()
        .enumerate()
    {
        let start = 155 + position * 52;
        for key in 0..10 {
            support::put16(definition, start + 4 + key * 3, 0xffff);
        }
        support::put16(definition, start + 4, column);
        definition[start + 6] = u8::from(position == 0 || !descending);
        support::put32(definition, start + 34, usage << 8);
        support::put32(definition, start + 38, root);
        definition[start + 46] = flags;
    }
    for (number, (data, kind)) in [(0, 1), (0, 2), (1, 2), (1, 0)].into_iter().enumerate() {
        let start = 259 + number * 28;
        definition[start..start + 28].fill(0);
        support::put32(definition, start + 4, number as u32);
        support::put32(definition, start + 8, data);
        definition[start + 23] = kind;
        if kind == 2 {
            definition[start + 12] = number as u8;
            support::put32(definition, start + 13, 3 - number as u32);
            support::put32(definition, start + 17, 10);
            definition[start + 21] = u8::from(cascade);
            definition[start + 22] = u8::from(cascade);
        }
    }
    let mut at = 371;
    for name in ["Primary", "ParentLink", "ChildLink", "ChildAlias"] {
        let name = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        support::put16(definition, at, name.len() as u16);
        at += 2;
        definition[at..at + name.len()].copy_from_slice(&name);
        at += name.len();
    }
    let rows = [(1_i32, None), (2, Some(1_i32)), (3, Some(1))]
        .into_iter()
        .map(|(id, parent)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(id.to_le_bytes());
            row.extend(parent.unwrap_or(0).to_le_bytes());
            row.push(if parent.is_some() { 3 } else { 1 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &rows));
    bytes[13 * 4096..14 * 4096].copy_from_slice(&index(&[Some(1), Some(2), Some(3)], false));
    bytes[14 * 4096..15 * 4096]
        .copy_from_slice(&support::data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0x20])]));
    bytes[15 * 4096..16 * 4096]
        .copy_from_slice(&support::data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0, 1])]));
    bytes[16 * 4096..17 * 4096].copy_from_slice(&index(&[None, Some(1), Some(1)], descending));
    let mut relationship = 8_u16.to_le_bytes().to_vec();
    for value in [if cascade { 0x1100_u32 } else { 0 }, 1, 0] {
        relationship.extend(value.to_le_bytes());
    }
    let mut offsets = vec![relationship.len() as u16];
    for name in ["SelfLink", "Items", "Items", "Id", "Score"] {
        relationship.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
        offsets.push(relationship.len() as u16);
    }
    for offset in offsets.into_iter().rev() {
        relationship.extend(offset.to_le_bytes());
    }
    relationship.extend(5_u16.to_le_bytes());
    relationship.push(0xff);
    support::put32(&mut bytes, 7 * 4096 + 16, 1);
    bytes[8 * 4096..9 * 4096]
        .copy_from_slice(&support::data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 2])]));
    bytes[9 * 4096..10 * 4096].copy_from_slice(&support::data_page(7, &[(0, relationship)]));
    bytes
}

pub(super) fn index(keys: &[Option<i32>], descending: bool) -> Vec<u8> {
    let mut page = vec![0xa7; 4096];
    page[..480].fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    support::put32(&mut page, 4, 10);
    let mut entries = keys.iter().copied().enumerate().collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        let order = a.1.cmp(&b.1);
        (if descending { order.reverse() } else { order }).then(a.0.cmp(&b.0))
    });
    let mut end = 0;
    for (row, key) in entries {
        let mut wire = vec![match (key, descending) {
            (None, false) => 0,
            (None, true) => 255,
            (Some(_), false) => 127,
            (Some(_), true) => 128,
        }];
        if let Some(key) = key {
            wire.extend(
                ((key as u32) ^ 0x80000000)
                    .to_be_bytes()
                    .map(|b| if descending { !b } else { b }),
            );
        }
        wire.extend(((12_u32 << 8) | row as u32).to_be_bytes());
        page[480 + end..480 + end + wire.len()].copy_from_slice(&wire);
        end += wire.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(&mut page, 2, (4096 - 480 - end) as u16);
    page
}
