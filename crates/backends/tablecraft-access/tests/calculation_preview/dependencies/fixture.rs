//! Independent malformed-cache/metadata controls; no producer qualification claim.
use super::super::{PAGE_BYTES, calculated_fixture, fixture as base, support};

const CATALOG_PAGE: usize = 4;
const OWNER_PAGE: usize = 10;
const DATA_PAGE: usize = 12;
const USAGE_PAGE: u32 = 11;
const FIRST_COLUMN: usize = 63;
const COLUMN_BYTES: usize = 25;
const VARIABLE_COUNT: usize = 43;
const VARIABLE_INDEX: usize = 7;
const FLAGS: usize = 15;
const EXTENDED_FLAGS: usize = 16;
const FIXED_OFFSET: usize = 21;
const CATALOG_SLOT: usize = 18;
const CATALOG_FIXED_BYTES: usize = 23;

fn encoded_name(text: &str) -> Vec<u8> {
    let bytes = text
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

pub(super) fn database(expressions: &[(&str, &str)], identities: &[i32]) -> Vec<u8> {
    let mut bytes = calculated_fixture::database(&[]);
    let mut properties = b"MR2\0".to_vec();
    properties.extend(block(
        0x80,
        &[encoded_name("Expression"), encoded_name("ResultType")].concat(),
    ));
    for (name, expression) in expressions {
        let text = expression
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let value = [
            ((text.len() + 8) as u16).to_le_bytes().as_slice(),
            &[0, 10, 0, 0],
            &(text.len() as u16).to_le_bytes(),
            &text,
        ]
        .concat();
        let name = encoded_name(name);
        let group = [
            ((name.len() + 4) as u32).to_le_bytes().as_slice(),
            &name,
            &value,
            &[9, 0, 0, 2, 1, 0, 1, 0, 4],
        ]
        .concat();
        properties.extend(block(1, &group));
    }
    let start = u16::from_le_bytes(
        bytes[CATALOG_PAGE * PAGE_BYTES + CATALOG_SLOT
            ..CATALOG_PAGE * PAGE_BYTES + CATALOG_SLOT + 2]
            .try_into()
            .unwrap(),
    ) as usize;
    let mut item = bytes[CATALOG_PAGE * PAGE_BYTES + start
        ..CATALOG_PAGE * PAGE_BYTES + start + CATALOG_FIXED_BYTES]
        .to_vec();
    item.extend((0x80000000 | properties.len() as u32).to_le_bytes());
    item.extend([0; 8]);
    item.extend(properties);
    item.extend((item.len() as u16).to_le_bytes());
    item.extend(23_u16.to_le_bytes());
    item.extend(16_u16.to_le_bytes());
    item.extend(2_u16.to_le_bytes());
    item.push(0x3f);
    bytes[CATALOG_PAGE * PAGE_BYTES..(CATALOG_PAGE + 1) * PAGE_BYTES].copy_from_slice(
        &support::data_page(
            2,
            &[
                (
                    0,
                    support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
                ),
                (
                    0,
                    support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
                ),
                (0, item),
            ],
        ),
    );
    let mut fields = vec![("Id", 4, 0, 4)];
    fields.extend(expressions.iter().map(|(name, _)| (*name, 4, 0, 39)));
    let mut definition = base::definition(&fields, identities.len() as u32, USAGE_PAGE << 8);
    support::put16(&mut definition, VARIABLE_COUNT, expressions.len() as u16);
    for index in 0..expressions.len() {
        let at = FIRST_COLUMN + (index + 1) * COLUMN_BYTES;
        definition[at + FLAGS] = 2;
        definition[at + EXTENDED_FLAGS] = 0xc0;
        support::put16(&mut definition, at + VARIABLE_INDEX, index as u16);
        support::put16(&mut definition, at + FIXED_OFFSET, 0);
    }
    bytes[OWNER_PAGE * PAGE_BYTES..(OWNER_PAGE + 1) * PAGE_BYTES].copy_from_slice(&definition);
    let rows = identities
        .iter()
        .map(|identity| {
            let count = expressions.len() + 1;
            let mut row = (count as u16).to_le_bytes().to_vec();
            row.extend(identity.to_le_bytes());
            // Every cached field has a one-byte invalid wrapper. Selected ordinary
            // input reads must prune these values while still checking row framing.
            row.extend(std::iter::repeat_n(0xff, expressions.len()));
            for position in (0..=expressions.len()).rev() {
                row.extend((6_u16 + position as u16).to_le_bytes());
            }
            row.extend((expressions.len() as u16).to_le_bytes());
            row.extend(std::iter::repeat_n(0xff, count.div_ceil(8)));
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(OWNER_PAGE as u32, &rows));
    bytes
}
