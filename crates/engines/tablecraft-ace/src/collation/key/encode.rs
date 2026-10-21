// SPDX-License-Identifier: Apache-2.0
// Rust adaptation of Jackcess General key encoding; see data/general/NOTICE.
//! Bounded primary/secondary/kana streams and native overflow-tail behavior.
use super::{
    buffer::{Buffer, Secondary, Work},
    tail,
    weights::{self, Weight},
};
use crate::{GeneralSortFamily, InspectError};

const PRIMARY_BYTES_PER_UNIT: usize = 8;
const SECONDARY_BYTES_PER_UNIT: usize = 4;
const UNPRINTABLE_BYTES_PER_UNIT: usize = 4;
const OUTPUT_BYTES_PER_UNIT: usize = 20;
const OUTPUT_FIXED_BYTES: usize = 16;
const LONG_REPEAT_PREFIX: u8 = 5;
const KANA_PREFIX: u8 = 0x7f;
const EMPTY_REPEAT: [u8; 2] = [0xff, 0xff];

pub(crate) struct ComparisonKey {
    pub bytes: Vec<u8>,
    pub primary_length: usize,
}

pub(crate) fn primary_byte_bound(
    family: GeneralSortFamily,
    unit: u16,
) -> Result<usize, InspectError> {
    let weight = weights::lookup(family, unit)?;
    Ok(if weight.kind == weights::REPEAT {
        PRIMARY_BYTES_PER_UNIT
    } else {
        weight.inline.len()
    })
}

pub(super) fn encode(
    family: GeneralSortFamily,
    units: &[u16],
    descending: bool,
    work: &mut Work<'_>,
) -> Result<Vec<u8>, InspectError> {
    Ok(encode_with_positions(family, units, descending, false, work)?.bytes)
}

pub(crate) fn encode_full(
    family: GeneralSortFamily,
    units: &[u16],
    work: &mut Work<'_>,
) -> Result<ComparisonKey, InspectError> {
    encode_with_positions(family, units, false, true, work)
}

fn encode_with_positions(
    family: GeneralSortFamily,
    units: &[u16],
    descending: bool,
    full: bool,
    work: &mut Work<'_>,
) -> Result<ComparisonKey, InspectError> {
    let count = units.len();
    // Full-string comparison widens positional suffixes; these bytes are never
    // written to native indexes. Checked arithmetic also bounds all smaller
    // per-unit capacities below before allocating or processing input.
    let extra = if full {
        size_of::<u64>() - size_of::<u16>()
    } else {
        0
    };
    let output = count
        .checked_mul(OUTPUT_BYTES_PER_UNIT + extra)
        .and_then(|size| size.checked_add(OUTPUT_FIXED_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    let mut primary = Buffer::new(output, work)?;
    let mut padded = Buffer::new(count * PRIMARY_BYTES_PER_UNIT, work)?;
    let mut has_padding = false;
    let mut secondary = Secondary {
        bytes: Buffer::new(count * SECONDARY_BYTES_PER_UNIT + 1, work)?,
        characters: 0,
    };
    let mut unprintable = Buffer::new(count * (UNPRINTABLE_BYTES_PER_UNIT + extra), work)?;
    let mut flags = Buffer::new(count, work)?;
    let mut copies = Buffer::new(count, work)?;
    let mut positions = work.vector::<usize>(count)?;
    let mut overrun = Buffer::new(count * SECONDARY_BYTES_PER_UNIT, work)?;
    let room = tail::ENTRIES_PER_UNIT * count;
    let mut characters = 0;
    let mut entries = 0;
    let mut last = [0; PRIMARY_BYTES_PER_UNIT];
    let mut last_length = 0;
    let mut last_flag = 0;
    let mut last_tertiary = &[][..];
    for unit in units {
        work.tick()?;
        let weight = weights::lookup(family, *unit)?;
        let old_characters = characters;
        let old_entries = entries;
        let repeated_empty = weight.kind == weights::REPEAT && last_length == 0;
        let mut inline = [0; PRIMARY_BYTES_PER_UNIT];
        let (length, character_count, entry_count) = if weight.kind == weights::REPEAT {
            if repeated_empty {
                inline[..EMPTY_REPEAT.len()].copy_from_slice(&EMPTY_REPEAT);
                (EMPTY_REPEAT.len(), 0, 1)
            } else {
                inline[..last_length].copy_from_slice(&last[..last_length]);
                if weight.prolonged && last_length == 2 && inline[0] == KANA_PREFIX {
                    inline[1] = weights::VOWELS[usize::from(inline[1])];
                }
                (last_length, 1, 1)
            }
        } else {
            if weight.inline.len() > inline.len() {
                return Err(InspectError::Malformed);
            }
            inline[..weight.inline.len()].copy_from_slice(weight.inline);
            (weight.inline.len(), weight.units, weight.entries)
        };
        let inline = &inline[..length];
        if !inline.is_empty() {
            primary.append(inline)?;
            characters += character_count;
            entries += entry_count;
            if characters > room {
                let tertiary = if weight.kind == weights::REPEAT {
                    last_tertiary
                } else {
                    weight.tertiary
                };
                for index in room.saturating_sub(old_characters)..character_count {
                    overrun.push(
                        tertiary
                            .get(index)
                            .copied()
                            .unwrap_or(tail::DEFAULT_TERTIARY),
                    )?;
                }
            }
            let padded_length = if family == GeneralSortFamily::General2010 {
                entry_count * size_of::<u16>()
            } else {
                0
            };
            if !has_padding && padded_length > inline.len() {
                padded.append(&primary.bytes()[..primary.len() - inline.len()])?;
                has_padding = true;
            }
            if has_padding {
                padded.append(inline)?;
                padded.fill(padded_length.saturating_sub(inline.len()), 0)?;
            }
            if !weight.prolonged {
                last_length = if character_count != 1 {
                    0
                } else if entry_count <= 1 {
                    last[..inline.len()].copy_from_slice(inline);
                    inline.len()
                } else {
                    last[..2].copy_from_slice(&[
                        LONG_REPEAT_PREFIX,
                        inline[inline.len() - size_of::<u16>()],
                    ]);
                    2
                };
            }
            if weight.kind != weights::REPEAT {
                last_flag = if character_count == 1 { weight.kana } else { 0 };
                last_tertiary = weight.tertiary;
            }
        }
        if weight.kind == weights::SIMPLE && weight.placeholder == tail::PLACEHOLDER {
            continue;
        }
        if repeated_empty {
            continue;
        }
        write_secondary(&weight, character_count, old_characters, &mut secondary)?;
        if !weight.unprintable.is_empty() {
            if full {
                tail::unprintable_full(old_entries, weight.unprintable, &mut unprintable)?;
            } else {
                tail::unprintable(old_entries, weight.unprintable, &mut unprintable)?;
            }
            last[0] = tail::UNPRINTABLE_KIND;
            if weight.unprintable.len() + 1 > last.len() {
                return Err(InspectError::Malformed);
            }
            last[1..weight.unprintable.len() + 1].copy_from_slice(weight.unprintable);
            last_length = weight.unprintable.len() + 1;
            last_flag = 0;
            last_tertiary = &[];
        }
        let flag = if weight.kind == weights::REPEAT {
            last_flag
        } else {
            weight.kana
        };
        if flag != 0 {
            if positions.len() >= count {
                return Err(InspectError::StructureLimit);
            }
            positions.push(old_characters);
            flags.push(flag)?;
            copies.push(if weight.prolonged {
                tail::COPY_PROLONGED
            } else if weight.kind == weights::REPEAT {
                tail::COPY_REPEAT
            } else {
                tail::COPY_DEFAULT
            })?;
        }
    }
    work.tick()?;
    let minimum = tail::tertiary_overrun(&mut flags, &positions, overrun.bytes(), room);
    tail::primary_overrun(
        primary.bytes(),
        has_padding.then(|| padded.bytes()),
        &mut secondary,
        characters,
        entries,
        count,
    )?;
    let primary_length = primary.len();
    tail::finish(
        &mut primary,
        &mut secondary,
        &unprintable,
        &mut flags,
        &mut copies,
        minimum,
        descending,
    )?;
    work.tick()?;
    Ok(ComparisonKey {
        bytes: primary.finish(),
        primary_length,
    })
}

fn write_secondary(
    weight: &Weight,
    characters: usize,
    offset: usize,
    secondary: &mut Secondary,
) -> Result<(), InspectError> {
    let mut placeholder = [0; SECONDARY_BYTES_PER_UNIT];
    let extra = if weight.extra.is_empty()
        && weight.modifier == 0
        && weight.placeholder != tail::PLACEHOLDER
    {
        if characters > placeholder.len() {
            return Err(InspectError::Malformed);
        }
        placeholder[..characters].fill(weight.placeholder);
        &placeholder[..characters]
    } else {
        weight.extra
    };
    if !extra.is_empty() || weight.modifier != 0 {
        secondary.write(offset, extra, weight.modifier, tail::PLACEHOLDER)?;
    }
    Ok(())
}
