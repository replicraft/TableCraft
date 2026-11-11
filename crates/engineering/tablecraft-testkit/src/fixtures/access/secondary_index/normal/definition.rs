//! Literal TDEF insertion retains all original descriptor/name order and tail bytes.
use crate::fixtures::access::{put16, put32};

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn long(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

pub(super) struct NewIndex<'a> {
    pub name: &'a str,
    pub column: u16,
    pub descending: bool,
    pub usage: u32,
    pub root: u32,
    pub distinct: u32,
}

pub(super) fn append(original: &[u8], request: NewIndex<'_>) -> Vec<u8> {
    append_with_logical(original, request, long(original, 47) as u32)
}

pub(super) fn append_with_logical(
    original: &[u8],
    request: NewIndex<'_>,
    logical_number: u32,
) -> Vec<u8> {
    let length = long(original, 8);
    let logical = long(original, 47);
    let physical = long(original, 51);
    let columns = 63 + physical * 12;
    let mut physical_start = columns + word(original, 45) * 25;
    for _ in 0..word(original, 45) {
        physical_start += 2 + word(original, physical_start);
    }
    let logical_start = physical_start + physical * 52;
    let names_start = logical_start + logical * 28;
    let tail = length - 2;
    let mut names_end = names_start;
    for _ in 0..logical {
        names_end += 2 + word(original, names_end);
    }
    assert_eq!(names_end, tail);
    assert_eq!(&original[tail..length], &[255, 255]);
    // NO_CLONE: The new active frame owns its reordered bytes while the
    // immutable original page remains available for every retained span and tail.
    let mut frame = original[..columns].to_vec();
    let mut count = [0; 12];
    put32(&mut count, 4, request.distinct);
    frame.extend(count);
    frame.extend(&original[columns..physical_start]);
    frame.extend(&original[physical_start..logical_start]);
    let mut descriptor = [0; 52];
    put32(&mut descriptor, 0, 1923);
    for slot in 0..10 {
        put16(&mut descriptor, 4 + slot * 3, 65535);
    }
    put16(&mut descriptor, 4, request.column);
    descriptor[6] = u8::from(!request.descending);
    put32(&mut descriptor, 34, request.usage);
    put32(&mut descriptor, 38, request.root);
    descriptor[46] = 128;
    frame.extend(descriptor);
    frame.extend(&original[logical_start..names_start]);
    let mut alias = [0; 28];
    put32(&mut alias, 0, 1625);
    put32(&mut alias, 4, logical_number);
    put32(&mut alias, 8, physical as u32);
    put32(&mut alias, 13, u32::MAX);
    frame.extend(alias);
    frame.extend(&original[names_start..tail]);
    let name = request.name.encode_utf16().collect::<Vec<_>>();
    frame.extend(((name.len() * 2) as u16).to_le_bytes());
    frame.extend(name.into_iter().flat_map(u16::to_le_bytes));
    frame.extend([255, 255]);
    let length = frame.len();
    assert!(length <= 4096);
    put32(&mut frame, 8, length as u32);
    put16(&mut frame, 2, 4096_usize.saturating_sub(length + 8) as u16);
    put32(&mut frame, 47, (logical + 1) as u32);
    put32(&mut frame, 51, (physical + 1) as u32);
    // NO_CLONE: The new frame is written into an independent complete page while
    // the original page remains the protected source and tail-byte oracle.
    let mut result = original.to_vec();
    result[..length].copy_from_slice(&frame);
    result
}
