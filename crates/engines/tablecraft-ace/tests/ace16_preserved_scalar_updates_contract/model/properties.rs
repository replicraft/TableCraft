//! Authored MR2 declarations, not copied donated property bytes.
use super::{records, schema};

const NAMES: [&str; 14] = [
    "GUID",
    "ColumnWidth",
    "ColumnOrder",
    "ColumnHidden",
    "DefaultView",
    "Expression",
    "ResultType",
    "AppendOnly",
    "TextFormat",
    "DefaultValue",
    "Required",
    "AllowZeroLength",
    "UnicodeCompression",
    "Format",
];

fn label(value: &str) -> Vec<u8> {
    let bytes = records::text(value);
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

fn property(name: usize, kind: u8, flags: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((payload.len() + 8) as u16).to_le_bytes().to_vec();
    bytes.push(flags);
    bytes.push(kind);
    bytes.extend((name as u16).to_le_bytes());
    bytes.extend((payload.len() as u16).to_le_bytes());
    bytes.extend(payload);
    bytes
}

fn group(kind: u16, name: &str, properties: &[Vec<u8>]) -> Vec<u8> {
    let label = label(name);
    let mut bytes = ((label.len() + 4) as u32).to_le_bytes().to_vec();
    bytes.extend(label);
    for property in properties {
        bytes.extend(property);
    }
    block(kind, &bytes)
}

pub(super) fn bag() -> Vec<u8> {
    let mut bytes = b"MR2\0".to_vec();
    let names = NAMES
        .iter()
        .flat_map(|name| label(name))
        .collect::<Vec<_>>();
    bytes.extend(block(0x80, &names));
    bytes.extend(group(
        0,
        "",
        &[property(0, 9, 1, &[0x91; 16]), property(4, 2, 0, &[2])],
    ));

    for (number, (name, ..)) in schema::FIELDS.iter().enumerate() {
        let mut properties = vec![
            property(0, 9, 1, &[number as u8 + 1; 16]),
            property(1, 3, 0, &(-1_i16).to_le_bytes()),
            property(2, 3, 0, &(number as i16).to_le_bytes()),
            property(3, 1, 0, &[0]),
        ];
        if number == 15 {
            properties.push(property(5, 12, 1, &records::text("[ID]+[Number]")));
            properties.push(property(6, 2, 1, &[4]));
        }
        if [2, 13].contains(&number) {
            properties.push(property(7, 1, 1, &[0]));
        }
        if number == 2 {
            properties.push(property(8, 2, 1, &[0]));
        }
        if [3, 9].contains(&number) {
            properties.push(property(9, 12, 1, &records::text("0")));
        }
        if [10, 11].contains(&number) {
            properties.push(property(9, 12, 1, &records::text("No")));
            properties.push(property(13, 10, 1, &records::text("Yes/No")));
        }
        if ![14, 15].contains(&number) {
            properties.push(property(10, 1, 1, &[0]));
        }
        if [1, 2, 13].contains(&number) {
            properties.push(property(11, 1, 1, &[1]));
        }
        if number == 1 {
            properties.push(property(12, 1, 1, &[1]));
        }
        bytes.extend(group(1, name, &properties));
    }
    bytes
}
