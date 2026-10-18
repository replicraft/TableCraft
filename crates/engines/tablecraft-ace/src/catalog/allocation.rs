use super::{bytes::u32_at, reader::Reader};
use crate::InspectError;
use std::io::{Read, Seek};

pub(super) fn pages<R: Read + Seek>(
    reader: &mut Reader<'_, R>,
    reference: u32,
) -> Result<Vec<u32>, InspectError> {
    let map = reader.row(reference >> 8, reference as u8, None)?;
    let mut pages = Vec::new();
    match map.first() {
        Some(0) => bitmap(
            reader,
            map.get(5..).ok_or(InspectError::Truncated)?,
            u32_at(&map, 1)?,
            &mut pages,
        )?,
        Some(1) => {
            if !(map.len() - 1).is_multiple_of(4) {
                return Err(InspectError::Malformed);
            }
            for (index, bytes) in map[1..].as_chunks::<4>().0.iter().enumerate() {
                reader.tick()?;
                let pointer = u32_at(bytes, 0)?;
                if pointer == 0 {
                    continue;
                }
                let page = reader.page(pointer)?;
                if page[..4] != [5, 1, 0, 0] {
                    return Err(InspectError::Malformed);
                }
                bitmap(
                    reader,
                    &page[4..],
                    (index * 4092 * 8)
                        .try_into()
                        .map_err(|_| InspectError::StructureLimit)?,
                    &mut pages,
                )?;
            }
        }
        _ => return Err(InspectError::Unsupported),
    }
    Ok(pages)
}
fn bitmap<R: Read + Seek>(
    reader: &mut Reader<'_, R>,
    bytes: &[u8],
    base: u32,
    pages: &mut Vec<u32>,
) -> Result<(), InspectError> {
    for (i, byte) in bytes.iter().enumerate() {
        reader.tick()?;
        for bit in 0..8 {
            if byte & (1 << bit) == 0 {
                continue;
            }
            let page = base
                .checked_add(
                    (i * 8 + bit)
                        .try_into()
                        .map_err(|_| InspectError::StructureLimit)?,
                )
                .ok_or(InspectError::Malformed)?;
            if page >= reader.header.page_count() {
                return Err(InspectError::Malformed);
            }
            if pages.len() >= 4096 || pages.contains(&page) {
                return Err(InspectError::StructureLimit);
            }
            reader
                .budget
                .charge(tablecraft_core::limits::Resource::Bytes, 4)?;
            pages.try_reserve(1).map_err(|_| InspectError::Allocation)?;
            pages.push(page);
        }
    }
    Ok(())
}
