//! Literal MR2 dictionary extension; old dictionary indices/groups stay exact.
use super::super::{number_group, wide, word};

const SIGNATURE_BYTES: usize = 4;
const DICTIONARY_LENGTH_OFFSET: usize = SIGNATURE_BYTES;
const DICTIONARY_NAMES_OFFSET: usize = 10;
const SHORT_BYTES: usize = size_of::<u16>();
const LONG_BYTES: usize = size_of::<u32>();
const GROUP_KIND: u16 = 1;
const DICTIONARY_KIND: u16 = 128;
const DICTIONARY_HEADER_BYTES: usize = LONG_BYTES + SHORT_BYTES;
const GROUP_NAME_HEADER_BYTES: usize = LONG_BYTES + SHORT_BYTES;
const GROUP_HEADER_BYTES: usize = 2 * (LONG_BYTES + SHORT_BYTES);
const BOOLEAN_ENTRY_BYTES: u16 = 9;
const PROPERTY_KIND: u8 = 1;
const BOOLEAN_TYPE: u8 = 1;

fn dictionary(body: &[u8]) -> (Vec<String>, &[u8]) {
    if body.is_empty() {
        return (Vec::new(), &[]);
    }
    assert_eq!(&body[..SIGNATURE_BYTES], b"MR2\0");
    let end = SIGNATURE_BYTES
        + u32::from_le_bytes(
            body[DICTIONARY_LENGTH_OFFSET..DICTIONARY_LENGTH_OFFSET + LONG_BYTES]
                .try_into()
                .unwrap(),
        ) as usize;
    let mut names = Vec::new();
    let mut position = DICTIONARY_NAMES_OFFSET;
    while position < end {
        let length = word(body, position);
        position += SHORT_BYTES;
        let units = body[position..position + length]
            .as_chunks::<SHORT_BYTES>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect::<Vec<_>>();
        names.push(String::from_utf16(&units).unwrap());
        position += length;
    }
    assert_eq!(position, end);
    (names, &body[end..])
}

fn encode(names: &[String], groups: &[u8], group: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::new();
    for name in names {
        let bytes = wide(name);
        encoded.extend((bytes.len() as u16).to_le_bytes());
        encoded.extend(bytes);
    }
    let mut body = b"MR2\0".to_vec();
    body.extend(((DICTIONARY_HEADER_BYTES + encoded.len()) as u32).to_le_bytes());
    body.extend(DICTIONARY_KIND.to_le_bytes());
    body.extend(encoded);
    body.extend(groups);
    body.extend(group);
    body
}

pub(in crate::fixtures::access) fn append_text(original: &[u8], name: &str) -> Vec<u8> {
    let (mut names, groups) = dictionary(original);
    for key in ["Required", "AllowZeroLength"] {
        if !names.iter().any(|name| name == key) {
            names.push(key.into());
        }
    }
    let mut entries = Vec::new();
    for (index, key) in names.iter().enumerate() {
        let value = match key.as_str() {
            "Required" => 0,
            "AllowZeroLength" => 255,
            _ => continue,
        };
        entries.extend(BOOLEAN_ENTRY_BYTES.to_le_bytes());
        entries.extend([PROPERTY_KIND, BOOLEAN_TYPE]);
        entries.extend((index as u16).to_le_bytes());
        entries.extend([1, 0, value]);
    }
    let label = wide(name);
    let mut group = ((GROUP_HEADER_BYTES + label.len() + entries.len()) as u32)
        .to_le_bytes()
        .to_vec();
    group.extend(GROUP_KIND.to_le_bytes());
    group.extend(((GROUP_NAME_HEADER_BYTES + label.len()) as u32).to_le_bytes());
    group.extend((label.len() as u16).to_le_bytes());
    group.extend(label);
    group.extend(entries);
    encode(&names, groups, &group)
}

pub(in crate::fixtures::access) fn append_number(original: &[u8], name: &str) -> Vec<u8> {
    let (mut names, groups) = dictionary(original);
    if !names.iter().any(|name| name == "DefaultValue") {
        names.push("DefaultValue".into());
    }
    if !names.iter().any(|name| name == "Required") {
        names.push("Required".into());
    }
    let dictionary = names.iter().map(String::as_str).collect::<Vec<_>>();
    let group = number_group(name, true, &dictionary);
    encode(&names, groups, &group)
}
