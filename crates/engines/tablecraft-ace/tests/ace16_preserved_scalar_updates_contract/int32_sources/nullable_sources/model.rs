//! Original-only nullable derivative of the qualified rich Int32 source.
use super::super::model as original;

const PAGE_BYTES: usize = 4096;
const DATA_PAGE: usize = 12;
const DIRECTORY: usize = 14;
const SLOT_BYTES: usize = 2;
const RECORD_HEADER: usize = 2;
const TARGET_OFFSET: usize = 8;
const MASK_BYTES: usize = 2;
const TARGET_BIT: u8 = 16;

pub(crate) const CASES: [&str; 7] = [
    "null_to_value",
    "null_to_minimum",
    "value_to_null",
    "null_noop",
    "mixed_rows",
    "required_refusal",
    "value_noop",
];

pub(crate) struct Case {
    pub before: Vec<u8>,
    pub expected: Option<Vec<u8>>,
    pub initial: [Option<i32>; 3],
    pub replacement: [Option<i32>; 3],
    pub requested_rows: usize,
    pub required: bool,
}

pub(crate) fn row(bytes: &[u8], ordinal: usize) -> (usize, usize, usize) {
    assert!(ordinal < 3);
    let page = DATA_PAGE * PAGE_BYTES;
    let at = page + DIRECTORY + ordinal * SLOT_BYTES;
    let start = usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()));
    let end = if ordinal == 0 {
        PAGE_BYTES
    } else {
        usize::from(u16::from_le_bytes(bytes[at - 2..at].try_into().unwrap()))
    };
    assert_eq!(start & 0xe000, 0);
    assert!(DIRECTORY + 3 * SLOT_BYTES <= start && start < end && end <= PAGE_BYTES);
    assert_eq!(
        &bytes[page + start..page + start + 2],
        &16_u16.to_le_bytes()
    );
    let payload = page + start + RECORD_HEADER + TARGET_OFFSET;
    let mask = page + end - MASK_BYTES;
    assert!(payload + 8 < mask);
    (payload, mask, page + end)
}

fn required(bytes: &mut [u8]) {
    let markers = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(offset, value)| (value == b"MR2\0").then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(markers.len(), 1);
    let mut at = markers[0] + 4;
    loop {
        let width = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let kind = u16::from_le_bytes(bytes[at + 4..at + 6].try_into().unwrap());
        assert!(width >= 6 && at + width <= bytes.len());
        if kind == 1 {
            let name_bytes = usize::from(u16::from_le_bytes(
                bytes[at + 10..at + 12].try_into().unwrap(),
            ));
            let name_start = at + 12;
            let name_end = name_start + name_bytes;
            let name = String::from_utf16(
                &bytes[name_start..name_end]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_le_bytes(*pair))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            if name == "Large_Number" {
                let mut property = name_end;
                while property < at + width {
                    let length = usize::from(u16::from_le_bytes(
                        bytes[property..property + 2].try_into().unwrap(),
                    ));
                    assert!(length >= 8 && property + length <= at + width);
                    if bytes[property + 4..property + 6] == 10_u16.to_le_bytes() {
                        assert_eq!(
                            &bytes[property..property + length],
                            &[9, 0, 1, 1, 10, 0, 1, 0, 0]
                        );
                        bytes[property + 8] = 1;
                        return;
                    }
                    property += length;
                }
                panic!("Required declaration missing");
            }
        }
        at += width;
    }
}

pub(crate) fn source(name: &str) -> Case {
    assert!(CASES.contains(&name));
    let mut before = original::source(6, 101).before;
    let initial = match name {
        "null_to_value" | "null_to_minimum" | "null_noop" => [None, Some(2000), Some(3000)],
        "mixed_rows" => [None, Some(2000), None],
        _ => [Some(1000), Some(2000), Some(3000)],
    };
    let replacement: [Option<i32>; 3] = match name {
        "null_to_value" => [Some(101), Some(2000), Some(3000)],
        "null_to_minimum" => [Some(-2_147_483_648), Some(2000), Some(3000)],
        "value_to_null" | "required_refusal" => [None, Some(2000), Some(3000)],
        "mixed_rows" => [Some(-101), None, Some(2_147_483_647)],
        _ => initial,
    };
    for (ordinal, value) in initial.iter().enumerate() {
        let (payload, mask, _) = row(&before, ordinal);
        assert_ne!(before[mask] & TARGET_BIT, 0);
        assert_eq!(
            &before[payload..payload + 4],
            &((ordinal as i32 + 1) * 1000).to_le_bytes()
        );
        if value.is_none() {
            before[mask] &= !TARGET_BIT;
            before[payload..payload + 4].copy_from_slice(&[0x91 + ordinal as u8, 0x82, 0x73, 0x64]);
        }
    }
    let is_required = name == "required_refusal";
    if is_required {
        required(&mut before);
    }
    let expected = if is_required {
        None
    } else {
        let mut expected = before.clone();
        for (ordinal, value) in replacement.iter().enumerate() {
            let (payload, mask, _) = row(&before, ordinal);
            if let Some(value) = value {
                expected[mask] |= TARGET_BIT;
                expected[payload..payload + 4].copy_from_slice(&value.to_le_bytes());
            } else {
                expected[mask] &= !TARGET_BIT;
            }
        }
        Some(expected)
    };
    Case {
        before,
        expected,
        initial,
        replacement,
        requested_rows: if name == "mixed_rows" { 3 } else { 1 },
        required: is_required,
    }
}
