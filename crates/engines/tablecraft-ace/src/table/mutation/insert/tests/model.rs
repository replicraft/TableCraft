use crate::{
    IndexCheckStatus, NativeImage, RowInsert, TableCheck, check_table, check_table_constraints,
    read_table,
    test_support::{base, map_growth},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

pub(super) const PAGE: usize = 4096;
pub(super) const HISTORY: u32 = 17;

pub(in crate::table::mutation::insert) fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

pub(super) struct NewRow {
    pub id: i32,
    pub binary: Option<Vec<u8>>,
}

impl NewRow {
    pub fn request(&self) -> RowInsert {
        RowInsert::new(vec![
            Value::Int32(self.id),
            self.binary
                .as_ref()
                .map_or(Value::Null, |value| Value::Binary(value.clone())),
        ])
    }
}

fn row(id: i32, binary: Option<&[u8]>) -> Vec<u8> {
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

fn inline(members: impl IntoIterator<Item = usize>) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for number in members {
        assert!(number < 512);
        bytes[5 + number / 8] |= 1 << (number % 8);
    }
    bytes
}

fn install(bytes: &mut [u8], number: usize, page: &[u8]) {
    assert_eq!(page.len(), PAGE);
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

fn leaf(original: Option<&[u8]>, entries: &[(i32, u32)]) -> Vec<u8> {
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

pub(in crate::table::mutation::insert) fn source(
    code: u8,
    alias: bool,
    full: bool,
    property: Option<(&str, &str)>,
) -> Vec<u8> {
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

pub(in crate::table::mutation::insert) fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

pub(in crate::table::mutation::insert) fn checked(
    bytes: &[u8],
    rows: usize,
    aliases: usize,
) -> TableCheck {
    let sequential = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(sequential.rows().len(), rows);
    let check = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(check.rows(), rows);
    assert_eq!(check.indexes().len(), aliases);
    assert!(
        check
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), rows);
    assert!(constraints.relationships().is_empty());
    check
}

fn count(page: &[u8]) -> usize {
    usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap()))
}
fn end(page: &[u8]) -> usize {
    let count = count(page);
    if count == 0 {
        PAGE
    } else {
        usize::from(
            u16::from_le_bytes(
                page[14 + 2 * (count - 1)..16 + 2 * (count - 1)]
                    .try_into()
                    .unwrap(),
            ) & 0x1fff,
        )
    }
}

pub(super) fn expected(before: &[u8], requests: &[NewRow]) -> (Vec<u8>, Vec<u32>) {
    let mut bytes = before.to_vec();
    let mut next = before.len() / PAGE;
    let mut owned = vec![12];
    let mut hints = vec![12];
    let mut keys = vec![(1, 12 << 8), (2, (12 << 8) | 1), (3, (12 << 8) | 2)];
    let mut references = Vec::new();
    for request in requests {
        let record = row(request.id, request.binary.as_deref());
        let mut destination = hints.iter().copied().max();
        if let Some(number) = destination {
            let page = &bytes[number * PAGE..(number + 1) * PAGE];
            if count(page) >= 255 || end(page) - 14 - count(page) * 2 < record.len() + 2 {
                hints.retain(|hint| *hint != number);
                destination = None;
            }
        }
        let number = destination.unwrap_or_else(|| {
            let number = next;
            next += 1;
            bytes.resize(next * PAGE, 0);
            let mut page = support::data_page(10, &[]);
            support::put16(&mut page, 2, 4082);
            install(&mut bytes, number, &page);
            owned.push(number);
            hints.push(number);
            number
        });
        let page = &mut bytes[number * PAGE..(number + 1) * PAGE];
        let slot = count(page);
        let record_end = end(page);
        let start = record_end - record.len();
        page[start..record_end].copy_from_slice(&record);
        support::put16(page, 14 + slot * 2, start as u16);
        support::put16(page, 12, (slot + 1) as u16);
        support::put16(page, 2, (start - 16 - slot * 2) as u16);
        let reference = (number as u32) << 8 | slot as u32;
        references.push(reference);
        keys.push((request.id, reference));
    }
    keys.sort();
    assert!(keys.windows(2).all(|pair| pair[0].0 != pair[1].0));
    let root = leaf(Some(&before[13 * PAGE..14 * PAGE]), &keys);
    install(&mut bytes, 13, &root);
    support::put32(&mut bytes, 10 * PAGE + 16, 3 + requests.len() as u32);
    support::put32(&mut bytes, 10 * PAGE + 67, HISTORY + requests.len() as u32);
    bytes[11 * PAGE + 4027..12 * PAGE].copy_from_slice(&inline(owned));
    bytes[11 * PAGE + 3958..11 * PAGE + 4027].copy_from_slice(&inline(hints));
    bytes[PAGE + 4027..2 * PAGE].copy_from_slice(&inline(next..512));
    (bytes, references)
}
