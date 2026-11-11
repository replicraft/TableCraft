/// Saved SELECT fragments composed onto an independent attachment table.
pub mod attachments;

/// Independent fitting SELECT expression BEFORE/AFTER byte oracles.
pub mod fitting_select;

use super::{data_page, mutation_fixture as base, put16, put32, record};

pub const PAGE_BYTES: usize = 4096;
pub const DEFINITION_PAGE: usize = 14;
pub const DATA_PAGE: usize = 16;
pub const QUERY_ID: i32 = i32::MIN + 123;
pub const EMPTY_ID: i32 = QUERY_ID + 1;

#[derive(Default)]
pub struct Fragment<'a> {
    pub object: Option<i32>,
    pub attribute: Option<u8>,
    pub expression: Option<&'a str>,
    pub flag: Option<i16>,
    pub extra: Option<i32>,
    pub name1: Option<&'a str>,
    pub name2: Option<&'a str>,
    pub order: Option<&'a [u8]>,
}
fn text(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn inline(value: &str) -> Vec<u8> {
    let bytes = text(value);
    let mut body = (0x8000_0000 | bytes.len() as u32).to_le_bytes().to_vec();
    body.extend([0; 8]);
    body.extend(bytes);
    body
}
pub fn database_with(fragments: &[Fragment<'_>]) -> Vec<u8> {
    database_with_query_flags(fragments, 0x80000030)
}
pub fn database_with_query_flags(fragments: &[Fragment<'_>], query_flags: u32) -> Vec<u8> {
    let mut bytes = base::database();
    bytes.resize(18 * PAGE_BYTES, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    put32(&mut bytes, 2 * PAGE_BYTES + 16, 6);
    let catalog = [
        ("MSysObjects", 2, 1, 0x80000002),
        ("MSysRelationships", 7, 1, 0x80000002),
        ("Items", 10, 1, 0),
        ("MSysQueries", 14, 1, 0x80000000),
        ("Saved", QUERY_ID, 5, query_flags),
        ("Empty", EMPTY_ID, 5, 0),
    ];
    let rows = catalog
        .into_iter()
        .map(|(name, id, kind, flags)| {
            (
                0,
                record(
                    id,
                    0,
                    kind,
                    flags,
                    &[b"\xff\xfe".as_slice(), name.as_bytes()].concat(),
                ),
            )
        })
        .collect::<Vec<_>>();
    bytes[4 * PAGE_BYTES..5 * PAGE_BYTES].copy_from_slice(&data_page(2, &rows));
    install_query_table(&mut bytes, fragments, DEFINITION_PAGE, 15, DATA_PAGE);
    bytes
}
fn install_query_table(
    bytes: &mut Vec<u8>,
    fragments: &[Fragment<'_>],
    definition_page: usize,
    map_page: usize,
    data_page: usize,
) {
    bytes.resize(bytes.len().max((data_page + 1) * PAGE_BYTES), 0);
    let mut definition = base::definition(
        &[
            ("ObjectId", 4, 0, 4),
            ("Attribute", 2, 4, 1),
            ("Order", 9, 0, 4),
            ("Name1", 10, 0, 510),
            ("Name2", 10, 0, 510),
            ("Expression", 12, 0, 0),
            ("Flag", 3, 5, 2),
            ("LvExtra", 4, 7, 4),
        ],
        fragments.len() as u32,
        (map_page as u32) << 8,
    );
    for (position, variable) in [(2, 0), (3, 1), (4, 2), (5, 3)] {
        let at = 63 + position * 25;
        definition[at + 15] = 2;
        put16(&mut definition, at + 7, variable);
    }
    put16(&mut definition, 43, 4);
    bytes[definition_page * PAGE_BYTES..(definition_page + 1) * PAGE_BYTES]
        .copy_from_slice(&definition);
    let rows: Vec<_> = fragments
        .iter()
        .map(|fragment| {
            let mut row = 8_u16.to_le_bytes().to_vec();
            row.extend(fragment.object.unwrap_or(0).to_le_bytes());
            row.push(fragment.attribute.unwrap_or(0));
            row.extend(fragment.flag.unwrap_or(0).to_le_bytes());
            row.extend(fragment.extra.unwrap_or(0).to_le_bytes());
            let mut offsets = vec![row.len() as u16];
            for value in [
                fragment.order.map(<[u8]>::to_vec),
                fragment.name1.map(text),
                fragment.name2.map(text),
                fragment.expression.map(inline),
            ] {
                if let Some(value) = value {
                    row.extend(value);
                }
                offsets.push(row.len() as u16);
            }
            for offset in offsets.into_iter().rev() {
                row.extend(offset.to_le_bytes());
            }
            row.extend(4_u16.to_le_bytes());
            row.push(
                u8::from(fragment.object.is_some())
                    | u8::from(fragment.attribute.is_some()) << 1
                    | u8::from(fragment.order.is_some()) << 2
                    | u8::from(fragment.name1.is_some()) << 3
                    | u8::from(fragment.name2.is_some()) << 4
                    | u8::from(fragment.expression.is_some()) << 5
                    | u8::from(fragment.flag.is_some()) << 6
                    | u8::from(fragment.extra.is_some()) << 7,
            );
            (0, row)
        })
        .collect::<Vec<_>>();
    let mut pages = Vec::new();
    let mut page = Vec::new();
    let mut occupied = 14;
    for row in rows {
        let cost = 2 + row.1.len();
        assert!(cost + 14 <= PAGE_BYTES);
        if occupied + cost > PAGE_BYTES {
            pages.push(std::mem::take(&mut page));
            occupied = 14;
        }
        occupied += cost;
        page.push(row);
    }
    pages.push(page);
    bytes.resize(bytes.len().max((data_page + pages.len()) * PAGE_BYTES), 0);
    let mut bitmap = vec![0; 5 + (data_page + pages.len()).div_ceil(8)];
    for page in data_page..data_page + pages.len() {
        bitmap[5 + page / 8] |= 1 << (page % 8);
    }
    bytes[map_page * PAGE_BYTES..(map_page + 1) * PAGE_BYTES]
        .copy_from_slice(&super::data_page(1, &[(0, bitmap)]));
    for (ordinal, rows) in pages.into_iter().enumerate() {
        let page = data_page + ordinal;
        bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES]
            .copy_from_slice(&super::data_page(definition_page as u32, &rows));
    }
}

pub fn database() -> Vec<u8> {
    database_with(&[
        Fragment {
            object: Some(QUERY_ID),
            attribute: Some(200),
            expression: Some("NeverRunMe()\t\n\0é🦀"),
            flag: Some(i16::MIN),
            extra: Some(i32::MIN),
            name1: Some("NULL"),
            name2: Some(""),
            order: Some(&[9, 8, 7, 6]),
        },
        Fragment {
            object: None,
            attribute: Some(255),
            ..Default::default()
        },
        Fragment {
            object: Some(QUERY_ID),
            order: Some(&[]),
            ..Default::default()
        },
    ])
}
