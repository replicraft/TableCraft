//! Literal MR2 declarations composed with actual independent record/image owners.
use super::super::ordinary_rows::{fixture, model::Row};
use tablecraft_testkit::access as raw;

pub const PAGE: usize = 4096;

pub struct Property {
    pub name: &'static str,
    pub kind: u8,
    pub flags: u8,
    pub data: Vec<u8>,
}

impl Property {
    pub fn text(name: &'static str, value: &str) -> Self {
        Self {
            name,
            kind: 10,
            flags: 0,
            data: value.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        }
    }

    pub fn boolean(name: &'static str, value: bool) -> Self {
        Self {
            name,
            kind: 1,
            flags: 0,
            data: vec![if value { 255 } else { 0 }],
        }
    }
}

pub struct Group {
    pub kind: u16,
    pub name: &'static str,
    pub properties: Vec<Property>,
}

pub fn field(name: &'static str, properties: Vec<Property>) -> Group {
    Group {
        kind: 1,
        name,
        properties,
    }
}

fn name(text: &str) -> Vec<u8> {
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

pub fn properties(bytes: &mut [u8], groups: &[Group]) {
    let mut names = Vec::new();
    for group in groups {
        for property in &group.properties {
            if !names.contains(&property.name) {
                names.push(property.name);
            }
        }
    }
    let mut blob = b"MR2\0".to_vec();
    let dictionary = names.iter().flat_map(|text| name(text)).collect::<Vec<_>>();
    blob.extend(block(128, &dictionary));
    for group in groups {
        let label = name(group.name);
        let mut body = ((label.len() + 4) as u32).to_le_bytes().to_vec();
        body.extend(label);
        for property in &group.properties {
            let number = names
                .iter()
                .position(|name| *name == property.name)
                .unwrap() as u16;
            body.extend(((property.data.len() + 8) as u16).to_le_bytes());
            body.extend([property.flags, property.kind]);
            body.extend(number.to_le_bytes());
            body.extend((property.data.len() as u16).to_le_bytes());
            body.extend(&property.data);
        }
        blob.extend(block(group.kind, &body));
    }
    let mut item = raw::record(10, 0, 1, 0, b"\xff\xfeItems");
    item.truncate(item.len() - 7);
    raw::put16(&mut item, 0, 6);
    let start = item.len() as u16;
    item.extend((0x80000000_u32 | blob.len() as u32).to_le_bytes());
    item.extend([0; 8]);
    item.extend(blob);
    item.extend((item.len() as u16).to_le_bytes());
    item.extend(start.to_le_bytes());
    item.extend([16, 0, 2, 0, 63]);
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
    bytes[4 * PAGE..5 * PAGE].copy_from_slice(&catalog);
}

pub fn ordinary(code: u8, expression: &str, required: bool) -> fixture::Case {
    let mut case = fixture::source(code, true, false, fixture::Index::Alias, false);
    if required {
        for row in &mut case.rows {
            row.score = Some("original".to_owned());
        }
        let records = case
            .rows
            .iter()
            .map(|row| (0, row.record(true)))
            .collect::<Vec<_>>();
        let used = records.iter().map(|(_, row)| row.len() + 2).sum::<usize>();
        let mut page = raw::data_page(10, &records);
        raw::put16(&mut page, 2, (4082 - used) as u16);
        case.before[12 * PAGE..13 * PAGE].copy_from_slice(&page);
    }
    properties(
        &mut case.before,
        &[
            field(
                "LaterText",
                vec![Property::boolean("AllowZeroLength", true)],
            ),
            field(
                "Score",
                vec![
                    Property::text("DefaultValue", expression),
                    Property::boolean("AllowZeroLength", true),
                    Property::boolean("Required", required),
                ],
            ),
        ],
    );
    case
}

pub fn additions() -> Vec<Row> {
    [Some("explicit"), None, Some("")]
        .into_iter()
        .enumerate()
        .map(|(position, value)| {
            let mut row = Row::rich(10 + position as i32);
            row.score = value.map(str::to_owned);
            row
        })
        .collect()
}

pub fn rules(code: u8, allow_empty: bool) -> fixture::Case {
    let mut case = ordinary(code, "\"fallback\"", false);
    properties(
        &mut case.before,
        &[
            field(
                "LaterText",
                vec![Property::boolean("AllowZeroLength", true)],
            ),
            field(
                "Score",
                vec![
                    Property::text("DefaultValue", "\"fallback\""),
                    Property::boolean("AllowZeroLength", allow_empty),
                ],
            ),
            field("ID", vec![Property::text("ValidationRule", ">0")]),
        ],
    );
    case
}
