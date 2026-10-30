//! Independent raw declaration framing reused by source builders.

pub const PAGE: usize = 4096;
const DEFINITION_HEADER: usize = 63;
const COLUMN_COUNT: usize = 45;
const LOGICAL_COUNT: usize = 47;
const PHYSICAL_COUNT: usize = 51;
const COUNTER_BYTES: usize = 12;
const COLUMN_BYTES: usize = 25;
const PHYSICAL_BYTES: usize = 52;
const LOGICAL_BYTES: usize = 28;

pub fn number(bytes: &[u8], offset: usize, width: usize) -> usize {
    let mut result = 0;
    for (position, &byte) in bytes[offset..offset + width].iter().enumerate() {
        result |= usize::from(byte) << (position * 8);
    }
    result
}

pub fn declaration_end(bytes: &[u8], definition: usize) -> usize {
    let base = definition * PAGE;
    let columns = number(bytes, base + COLUMN_COUNT, 2);
    let physical = number(bytes, base + PHYSICAL_COUNT, 4);
    let logical = number(bytes, base + LOGICAL_COUNT, 4);
    let mut position = base + DEFINITION_HEADER + physical * COUNTER_BYTES;
    position += columns * COLUMN_BYTES;
    for _ in 0..columns {
        let width = number(bytes, position, 2);
        assert_eq!(width % 2, 0);
        position += 2 + width;
    }
    position += physical * PHYSICAL_BYTES + logical * LOGICAL_BYTES;
    for _ in 0..logical {
        let width = number(bytes, position, 2);
        assert_eq!(width % 2, 0);
        position += 2 + width;
    }
    assert!(position + 2 <= base + PAGE);
    position
}
