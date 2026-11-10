//! Independent MR2 group removal and renaming preserve dictionary identity.
use super::{put16, put32};

const SHORT_BYTES: usize = 2;
const LONG_BYTES: usize = 4;
const NAME_LIST: usize = 128;
const COLUMN_GROUP: usize = 1;
const DICTIONARY_OFFSET: usize = 4;
const DICTIONARY_KIND: usize = 8;
const GROUP_KIND: usize = 4;
const GROUP_NAME_FRAME: usize = 6;
const GROUP_NAME_LENGTH: usize = 10;
const GROUP_NAME: usize = 12;

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn long(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

fn name(bytes: &[u8]) -> String {
    let units = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect::<Vec<_>>();
    String::from_utf16(&units).unwrap()
}
pub(in crate::fixtures::access) fn without_group(body: &[u8], selected: &str) -> Vec<u8> {
    assert_eq!(&body[..DICTIONARY_OFFSET], b"MR2\0");
    assert_eq!(word(body, DICTIONARY_KIND), NAME_LIST);
    let mut position = DICTIONARY_OFFSET + long(body, DICTIONARY_OFFSET);
    let mut result = body[..position].to_vec();
    let mut removed = 0;
    while position < body.len() {
        let length = long(body, position);
        let name_bytes = word(body, position + GROUP_NAME_LENGTH);
        assert_eq!(
            long(body, position + GROUP_NAME_FRAME),
            name_bytes + LONG_BYTES + SHORT_BYTES
        );
        assert!(GROUP_NAME + name_bytes <= length);
        let label = name(&body[position + GROUP_NAME..position + GROUP_NAME + name_bytes]);
        if word(body, position + GROUP_KIND) == COLUMN_GROUP && label == selected {
            removed += 1;
        } else {
            result.extend_from_slice(&body[position..position + length]);
        }
        position += length;
    }
    assert_eq!(position, body.len());
    assert_eq!(removed, 1);
    result
}

pub(in crate::fixtures::access) fn rename(body: &[u8], old: &str, new: &str) -> Vec<u8> {
    assert_eq!(&body[..4], b"MR2\0");
    let mut position = 4 + long(body, 4);
    let mut result = body[..position].to_vec();
    let mut changed = 0;
    let wide = new
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    while position < body.len() {
        let length = long(body, position);
        let name_bytes = word(body, position + 10);
        let end = position + 12 + name_bytes;
        if word(body, position + 4) == 1 && name(&body[position + 12..end]) == old {
            let mut group = body[position..position + 12].to_vec();
            group.extend(&wide);
            group.extend_from_slice(&body[end..position + length]);
            let size = group.len();
            put32(&mut group, 0, size as u32);
            put32(&mut group, 6, (wide.len() + 6) as u32);
            put16(&mut group, 10, wide.len() as u16);
            result.extend(group);
            changed += 1;
        } else {
            result.extend_from_slice(&body[position..position + length]);
        }
        position += length;
    }
    assert_eq!(changed, 1);
    result
}
