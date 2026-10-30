//! Original ordinary INSERT seed construction, without producer code.
use super::map_growth;
use super::mutation_fixture as base;
use crate::access as support;

pub const PAGE: usize = 4096;
pub const HISTORY: u32 = 17;

pub fn row(id: i32, binary: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = vec![2, 0];
    bytes.extend(id.to_le_bytes());
    if let Some(value) = binary {
        bytes.extend_from_slice(value);
    }
    let end = bytes.len() as u16;
    bytes.extend(end.to_le_bytes());
    bytes.extend([6, 0, 1, 0, if binary.is_some() { 3 } else { 1 }]);
    bytes
}

pub fn inline(members: impl IntoIterator<Item = usize>) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for number in members {
        assert!(number < 512);
        bytes[5 + number / 8] |= 1 << (number % 8);
    }
    bytes
}

pub fn install(bytes: &mut [u8], number: usize, page: &[u8]) {
    assert_eq!(page.len(), PAGE);
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

pub fn leaf(original: Option<&[u8]>, entries: &[(i32, u32)]) -> Vec<u8> {
    let encoded = entries
        .iter()
        .map(|(key, reference)| {
            [
                vec![127],
                ((*key as u32) ^ 0x80000000).to_be_bytes().to_vec(),
                reference.to_be_bytes().to_vec(),
            ]
            .concat()
        })
        .collect::<Vec<_>>();
    let mut common = encoded.first().map_or(0, Vec::len);
    for value in &encoded[1..] {
        common = encoded[0][..common]
            .iter()
            .zip(value)
            .take_while(|(left, right)| left == right)
            .count();
    }
    let seed = original.map_or(0, |bytes| {
        usize::from(u16::from_le_bytes(bytes[24..26].try_into().unwrap()))
    });
    let saved = encoded.len().saturating_sub(1);
    let mut prefix = seed.min(common);
    if encoded.len() * 9 - prefix * saved > 3616 {
        prefix = common;
    }
    let active = encoded.len() * 9 - prefix * saved;
    assert!(active <= 3616);
    let mut bytes = original.map_or_else(|| vec![0xa7; PAGE], <[u8]>::to_vec);
    bytes[..480].fill(0);
    bytes[..2].copy_from_slice(&[4, 1]);
    support::put16(&mut bytes, 2, (3616 - active) as u16);
    support::put32(&mut bytes, 4, 10);
    support::put16(&mut bytes, 24, prefix as u16);
    let mut end = 0;
    for (position, value) in encoded.iter().enumerate() {
        let suffix = &value[if position == 0 { 0 } else { prefix }..];
        bytes[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        bytes[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, active);
    if let Some(original) = original {
        assert_eq!(&bytes[480 + active..], &original[480 + active..]);
    }
    bytes
}

pub fn source(code: u8, alias: bool, full: bool, property: Option<(&str, &str)>) -> Vec<u8> {
    let (property, value) = property.unwrap_or(("Caption", "ordinary Binary insert"));
    let mut bytes = base::with_rule(property, value);
    bytes.resize(16 * PAGE, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    let mut definition = base::definition(&[("ID", 4, 0, 4), ("Score", 9, 0, 255)], 3, 11 << 8);
    definition[88 + 15] = 2;
    support::put16(&mut definition, 88 + 7, 0);
    support::put16(&mut definition, 43, 1);
    definition.copy_within(63..131, 75);
    definition[63..75].fill(0);
    support::put32(&mut definition, 67, HISTORY);
    support::put32(&mut definition, 47, if alias { 2 } else { 1 });
    support::put32(&mut definition, 51, 1);
    support::put32(&mut definition, 59, (11 << 8) | 1);
    for key in 0..10 {
        support::put16(&mut definition, 147 + 3 * key, 0xffff);
    }
    support::put16(&mut definition, 147, 0);
    definition[149] = 1;
    support::put32(&mut definition, 177, 14 << 8);
    support::put32(&mut definition, 181, 13);
    definition[189] = 0x89;
    definition[218] = 1;
    if alias {
        support::put32(&mut definition, 223 + 4, 1);
    }
    let mut at = 195 + if alias { 56 } else { 28 };
    for name in if alias {
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
    let mut rows = vec![
        (0, row(1, None)),
        (0, row(2, Some(&[]))),
        (0, row(3, Some(&[0x80, 0, 0xff]))),
    ];
    if full {
        rows.push((0x8000, vec![0x9d; 3800]));
    }
    let free = PAGE - 14 - rows.len() * 2 - rows.iter().map(|(_, row)| row.len()).sum::<usize>();
    let mut data = support::data_page(10, &rows);
    support::put16(&mut data, 2, free as u16);
    install(&mut bytes, 12, &data);
    install(
        &mut bytes,
        11,
        &support::data_page(0, &[(0, inline([12])), (0, inline([12]))]),
    );
    install(&mut bytes, 14, &support::data_page(0, &[(0, inline([13]))]));
    install(
        &mut bytes,
        1,
        &support::data_page(1, &[(0, inline(16..512))]),
    );
    install(
        &mut bytes,
        13,
        &leaf(
            None,
            &[(1, 12 << 8), (2, (12 << 8) | 1), (3, (12 << 8) | 2)],
        ),
    );
    for number in [2, 7, 10] {
        let end = map_growth::model::declaration_end(&bytes, number);
        support::put16(&mut bytes, end, 0xffff);
    }
    bytes
}
