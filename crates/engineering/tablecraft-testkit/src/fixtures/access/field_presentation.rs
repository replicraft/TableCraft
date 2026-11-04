//! Independent narrow-MR2 Score properties and mask-only Boolean DATA fixtures.
use super::{self as raw, mutation_fixture, wide_properties};

const PAGE: usize = 4096;

/// Encode one narrow-MR2 property, with explicit flags and native carrier.
pub fn property(index: u16, flags: u8, kind: u8, value: &[u8]) -> Vec<u8> {
    let mut bytes = u16::try_from(value.len() + 8)
        .unwrap()
        .to_le_bytes()
        .to_vec();
    bytes.extend([flags, kind]);
    bytes.extend(index.to_le_bytes());
    bytes.extend(u16::try_from(value.len()).unwrap().to_le_bytes());
    bytes.extend(value);
    bytes
}

/// Encode a named Score group, including explicit non-column negative cases.
pub fn score_group(kind: u16, properties: &[Vec<u8>]) -> Vec<u8> {
    let name = wide_properties::name("Score");
    let mut body = u32::try_from(name.len() + 4)
        .unwrap()
        .to_le_bytes()
        .to_vec();
    body.extend(name);
    for property in properties {
        body.extend(property);
    }
    wide_properties::block(kind, &body)
}

/// Attach literal property blocks to Items, optionally making Score a Boolean.
pub fn database(blocks: &[Vec<u8>], boolean: bool) -> Vec<u8> {
    let mut bytes = mutation_fixture::database();
    let properties = wide_properties::bag(blocks);
    let mut item = raw::record(10, 0, 1, 0, b"\xff\xfeItems");
    item.truncate(item.len() - 7);
    raw::put16(&mut item, 0, 6);
    let property_start = u16::try_from(item.len()).unwrap();
    item.extend((0x80000000 | properties.len() as u32).to_le_bytes());
    item.extend([0; 8]);
    item.extend(properties);
    item.extend(u16::try_from(item.len()).unwrap().to_le_bytes());
    item.extend(property_start.to_le_bytes());
    item.extend(16_u16.to_le_bytes());
    item.extend(2_u16.to_le_bytes());
    item.push(0x3f);
    let catalog = [
        (0, raw::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects")),
        (
            0,
            raw::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        ),
        (0, item),
    ];
    bytes[4 * PAGE..5 * PAGE].copy_from_slice(&raw::data_page(2, &catalog));

    if boolean {
        let definition =
            mutation_fixture::definition(&[("Id", 4, 0, 4), ("Score", 1, 0, 0)], 2, 11 << 8);
        bytes[10 * PAGE..11 * PAGE].copy_from_slice(&definition);
        let rows = [(1_i32, 1_u8), (2_i32, 3_u8)]
            .into_iter()
            .map(|(id, mask)| {
                let mut row = 2_u16.to_le_bytes().to_vec();
                row.extend(id.to_le_bytes());
                row.push(mask);
                (0, row)
            })
            .collect::<Vec<_>>();
        bytes[12 * PAGE..13 * PAGE].copy_from_slice(&raw::data_page(10, &rows));
    }

    bytes
}

/// Build the original single-property declaration cases without a native encoder.
pub fn single(key: &str, flags: u8, native_type: u8, text: &str, boolean: bool) -> Vec<u8> {
    database(
        &[
            wide_properties::names(&[key]),
            score_group(
                1,
                &[property(
                    0,
                    flags,
                    native_type,
                    &wide_properties::text(text),
                )],
            ),
        ],
        boolean,
    )
}
