use super::support;
use tablecraft_testkit::access::mutation_fixture as fixture;

// Original independent golden definition/entry bytes. An aliased logical index
// deliberately shares the same physical owner, not a second patchable page.
pub fn database(descending: bool, alias: bool, nullable: bool) -> Vec<u8> {
    with_rule(descending, alias, nullable, "Caption", "indexed edits")
}
pub fn with_rule(descending: bool, alias: bool, nullable: bool, key: &str, text: &str) -> Vec<u8> {
    let mut bytes = fixture::with_rule(key, text);
    bytes.resize(15 * 4096, 0);
    let values = if nullable {
        vec![None, Some(20), Some(30)]
    } else {
        vec![Some(10), Some(20), Some(30)]
    };
    let definition = &mut bytes[10 * 4096..11 * 4096];
    definition.copy_within(63..131, 75);
    definition[63..75].fill(0);
    support::put32(definition, 67, 3);
    support::put32(definition, 47, if alias { 2 } else { 1 });
    support::put32(definition, 51, 1);
    for key in 0..10 {
        support::put16(definition, 143 + 4 + key * 3, 0xffff);
    }
    support::put16(definition, 147, 1);
    definition[149] = u8::from(!descending);
    support::put32(definition, 177, 14 << 8);
    support::put32(definition, 181, 13);
    definition[189] = 0x81;
    definition[195 + 23] = u8::from(!nullable);
    if alias {
        support::put32(definition, 195 + 28 + 4, 1);
    }
    let mut names = 195 + 28 * if alias { 2 } else { 1 };
    for name in if alias {
        vec!["ScoreKey", "ScoreAlias"]
    } else {
        vec!["ScoreKey"]
    } {
        let data = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        support::put16(definition, names, data.len() as u16);
        names += 2;
        definition[names..names + data.len()].copy_from_slice(&data);
        names += data.len();
    }
    let rows = values
        .iter()
        .enumerate()
        .map(|(ordinal, key)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend((ordinal as i32 + 1).to_le_bytes());
            row.extend(key.unwrap_or(0_i32).to_le_bytes());
            row.push(if key.is_some() { 3 } else { 1 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &rows));
    bytes[14 * 4096..15 * 4096]
        .copy_from_slice(&support::data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0x20])]));
    let mut entries = values.iter().copied().enumerate().collect::<Vec<_>>();
    entries.sort_by(|(a, x), (b, y)| {
        let order = x.cmp(y);
        (if descending { order.reverse() } else { order }).then(a.cmp(b))
    });
    let prefix = if nullable { 0 } else { 4 };
    let page = &mut bytes[13 * 4096..14 * 4096];
    page.fill(0xa7);
    page[..480].fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    support::put32(page, 4, 10);
    support::put16(page, 24, prefix);
    let mut first = Vec::new();
    let mut end = 0;
    for (position, (slot, key)) in entries.into_iter().enumerate() {
        let mut data = vec![match (key, descending) {
            (None, false) => 0,
            (None, true) => 0xff,
            (Some(_), false) => 0x7f,
            (Some(_), true) => 0x80,
        }];
        if let Some(key) = key {
            data.extend(
                ((key as u32) ^ 0x80000000)
                    .to_be_bytes()
                    .map(|byte| if descending { !byte } else { byte }),
            );
        }
        data.extend(((12_u32 << 8) | slot as u32).to_be_bytes());
        let shared = if position == 0 {
            first = data.clone();
            0
        } else {
            prefix as usize
        };
        assert_eq!(data[..shared], first[..shared]);
        page[480 + end..480 + end + data.len() - shared].copy_from_slice(&data[shared..]);
        end += data.len() - shared;
        page[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(page, 2, (4096 - 480 - end) as u16);
    bytes
}

pub fn node_database() -> Vec<u8> {
    let mut bytes = database(false, false, false);
    bytes.resize(17 * 4096, 0);
    bytes[14 * 4096..15 * 4096].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0xa0, 1])],
    ));
    let entry = |key: i32, slot: u32| {
        let mut data = vec![0x7f];
        data.extend(((key as u32) ^ 0x80000000).to_be_bytes());
        data.extend(((12_u32 << 8) | slot).to_be_bytes());
        data
    };
    let page = |leaf: bool, entries: Vec<Vec<u8>>, previous: u32, next: u32, tail: u32| {
        let mut page = vec![0; 4096];
        page[..2].copy_from_slice(&[if leaf { 4 } else { 3 }, 1]);
        page[26] = u8::from(!leaf);
        support::put32(&mut page, 4, 10);
        support::put32(&mut page, 12, previous);
        support::put32(&mut page, 16, next);
        support::put32(&mut page, 20, tail);
        let mut end = 0;
        for entry in entries {
            page[480 + end..480 + end + entry.len()].copy_from_slice(&entry);
            end += entry.len();
            page[27 + end / 8] |= 1 << (end % 8);
        }
        support::put16(&mut page, 2, (4096 - 480 - end) as u16);
        page
    };
    let mut separator = entry(20, 1);
    separator.extend(15_u32.to_be_bytes());
    bytes[13 * 4096..14 * 4096].copy_from_slice(&page(false, vec![separator], 0, 0, 16));
    bytes[15 * 4096..16 * 4096].copy_from_slice(&page(
        true,
        vec![entry(10, 0), entry(20, 1)],
        0,
        16,
        0,
    ));
    bytes[16 * 4096..17 * 4096].copy_from_slice(&page(true, vec![entry(30, 2)], 15, 0, 0));
    bytes
}
