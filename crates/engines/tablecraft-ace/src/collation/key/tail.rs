// SPDX-License-Identifier: Apache-2.0
// Rust adaptation of Jackcess General key encoding; see data/general/NOTICE.
use super::buffer::{Buffer, Secondary};
use crate::InspectError;

pub(super) const END: u8 = 1;
pub(super) const EXTRA_END: u8 = 0;
pub(super) const PLACEHOLDER: u8 = 2;
pub(super) const DEFAULT_TERTIARY: u8 = 1;
pub(super) const ENTRIES_PER_UNIT: usize = 3;
const ENTRY_BYTES: usize = 2;
const UNPRINTABLE_START: usize = 7;
const UNPRINTABLE_STRIDE: usize = 4;
const UNPRINTABLE_FLAG: u16 = 0x8000;
const FULL_UNPRINTABLE_FLAG: u64 = 1 << (u64::BITS - 1);
pub(super) const UNPRINTABLE_KIND: u8 = 6;
const PACK_PREFIX: u8 = 0x80;
const PACKED_PER_BYTE: usize = 3;
const BITS_PER_CODE: usize = 2;
const KANA_DEFAULT: u8 = 3;
pub(super) const COPY_DEFAULT: u8 = 1;
pub(super) const COPY_REPEAT: u8 = 2;
pub(super) const COPY_PROLONGED: u8 = 3;
const SUFFIX_FLAG: u8 = 0xff;
const SUFFIX_MIDFIX: u8 = 2;
const SUFFIX_REPEAT: u8 = 0x80;
const SUFFIX_GROUP_UNITS: usize = 7;

pub(super) fn unprintable(
    offset: usize,
    bytes: &[u8],
    output: &mut Buffer,
) -> Result<(), InspectError> {
    let position = u16::try_from(UNPRINTABLE_START + UNPRINTABLE_STRIDE * offset)
        .map_err(|_| InspectError::StructureLimit)?
        | UNPRINTABLE_FLAG;
    output.append(&position.to_be_bytes())?;
    output.push(UNPRINTABLE_KIND)?;
    output.append(bytes)
}
pub(super) fn unprintable_full(
    offset: usize,
    bytes: &[u8],
    output: &mut Buffer,
) -> Result<(), InspectError> {
    let position = u64::try_from(offset)
        .ok()
        .and_then(|offset| offset.checked_mul(UNPRINTABLE_STRIDE as u64))
        .and_then(|offset| offset.checked_add(UNPRINTABLE_START as u64))
        .filter(|offset| *offset < FULL_UNPRINTABLE_FLAG)
        .ok_or(InspectError::StructureLimit)?;
    output.append(&(position | FULL_UNPRINTABLE_FLAG).to_be_bytes())?;
    output.push(UNPRINTABLE_KIND)?;
    output.append(bytes)
}
pub(super) fn primary_overrun(
    primary: &[u8],
    padded: Option<&[u8]>,
    secondary: &mut Secondary,
    characters: usize,
    entries: usize,
    units: usize,
) -> Result<(), InspectError> {
    let length = ENTRY_BYTES * entries.saturating_sub(ENTRIES_PER_UNIT * units);
    if length == 0 {
        return Ok(());
    }
    secondary.pad(characters, PLACEHOLDER)?;
    let weights = padded.unwrap_or(primary);
    let length = length.min(secondary.bytes.len()).min(weights.len());
    secondary.bytes.bytes_mut()[..length].copy_from_slice(&weights[weights.len() - length..]);
    Ok(())
}
pub(super) fn tertiary_overrun(
    flags: &mut Buffer,
    positions: &[usize],
    overrun: &[u8],
    room: usize,
) -> usize {
    let mut minimum = 0;
    for (i, weight) in overrun.iter().enumerate().take(flags.len()) {
        if room + i >= positions[i] {
            flags.bytes_mut()[i] = *weight;
            minimum = i + 1;
        }
    }
    minimum
}
fn pack(codes: &[u8], output: &mut Buffer) -> Result<(), InspectError> {
    for group in codes.chunks(PACKED_PER_BYTE) {
        let mut byte = PACK_PREFIX;
        for (index, code) in group.iter().enumerate() {
            byte |= code.wrapping_shl(((PACKED_PER_BYTE - 1 - index) * BITS_PER_CODE) as u32);
        }
        output.push(byte)?;
    }
    Ok(())
}
pub(super) fn finish(
    primary: &mut Buffer,
    secondary: &mut Secondary,
    unprintable: &Buffer,
    flags: &mut Buffer,
    copies: &mut Buffer,
    minimum: usize,
    descending: bool,
) -> Result<(), InspectError> {
    primary.push(END)?;
    secondary.bytes.trim(|b| b <= PLACEHOLDER, 0);
    let has_flags = flags.len() != 0;
    let has_unprintable = unprintable.len() != 0;
    primary.append(secondary.bytes.bytes())?;
    if has_flags || has_unprintable {
        primary.append(&[END, END])?;
        if has_flags {
            let count = flags.len();
            flags.trim(|b| b == KANA_DEFAULT, minimum);
            pack(flags.bytes(), primary)?;
            primary.push(SUFFIX_FLAG)?;
            copies.trim(|b| b == COPY_DEFAULT, 0);
            pack(copies.bytes(), primary)?;
            primary.push(SUFFIX_MIDFIX)?;
            primary.fill(count.div_ceil(SUFFIX_GROUP_UNITS), SUFFIX_REPEAT)?;
            primary.push(SUFFIX_FLAG)?;
            primary.fill(count.div_ceil(SUFFIX_GROUP_UNITS), SUFFIX_REPEAT)?;
            if has_unprintable {
                primary.push(SUFFIX_FLAG)?;
            }
        }
        if has_unprintable {
            primary.push(END)?;
            primary.append(unprintable.bytes())?;
        }
    }
    if descending {
        primary.push(EXTRA_END)?;
        for byte in primary.bytes_mut() {
            *byte = !*byte;
        }
    }
    primary.push(EXTRA_END)
}
