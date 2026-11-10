//! Logical table-definition bytes bounded by native framing and source geometry.
use super::{
    bytes::u32_at,
    page::{PAGE_BYTES, TABLE_DEFINITION_SIGNATURE},
    reader::{Reader, visited::Visited},
};
use crate::{InspectError, NativeReadSource};
use std::ops::Deref;

pub(crate) const PAGE_HEADER_BYTES: usize = 2 * size_of::<u16>() + size_of::<u32>();
pub(crate) const NEXT_PAGE_OFFSET: usize = PAGE_HEADER_BYTES - size_of::<u32>();
pub(crate) const LENGTH_OFFSET: usize = PAGE_HEADER_BYTES;
const MIN_DEFINITION_BYTES: usize = LENGTH_OFFSET + size_of::<u32>();
const CONTINUATION_BYTES: usize = PAGE_BYTES - PAGE_HEADER_BYTES;

/// Retain the ordinary one-page path on the stack without a heap allocation.
pub(crate) struct Definition {
    first: [u8; PAGE_BYTES],
    joined: Vec<u8>,
    length: usize,
}

impl Deref for Definition {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        if self.joined.is_empty() {
            &self.first[..self.length]
        } else {
            &self.joined
        }
    }
}

pub(crate) fn read<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    root: u32,
) -> Result<Definition, InspectError> {
    read_with_pages(reader, root, |_, _| Ok(()))
}

/// Visit each checked physical definition page while retaining logical bytes.
/// A later chain or visitor error returns no definition; callers must not treat
/// a partial visitor inventory as a completed storage proof.
pub(crate) fn read_with_pages<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    root: u32,
    mut visit: impl FnMut(u32, &mut Reader<'_, R>) -> Result<(), InspectError>,
) -> Result<Definition, InspectError> {
    let first = reader.page(root)?;
    validate(&first)?;
    let length = u32_at(&first, LENGTH_OFFSET)? as usize;
    if length < MIN_DEFINITION_BYTES {
        return Err(InspectError::Malformed);
    }
    // Page zero is the file header, so it cannot hold a definition chunk.
    let available = u64::from(reader.header.page_count()).saturating_sub(1);
    let maximum = PAGE_BYTES as u64 + available.saturating_sub(1) * CONTINUATION_BYTES as u64;
    if length as u64 > maximum {
        return Err(InspectError::Truncated);
    }
    let mut next = u32_at(&first, NEXT_PAGE_OFFSET)?;
    let mut result = Definition {
        first,
        joined: Vec::new(),
        length,
    };
    if length <= PAGE_BYTES {
        if next != 0 {
            return Err(InspectError::Malformed);
        }
        visit(root, reader)?;
        reader.tick()?;
        return Ok(result);
    }
    visit(root, reader)?;
    let mut visited = Visited::new();
    visited.insert(root, reader)?;
    append(&mut result.joined, &result.first, reader)?;
    while result.joined.len() < length {
        reader.tick()?;
        if next == 0 {
            return Err(InspectError::Truncated);
        }
        visited.insert(next, reader)?;
        let page = reader.page(next)?;
        validate(&page)?;
        visit(next, reader)?;
        next = u32_at(&page, NEXT_PAGE_OFFSET)?;
        let take = (length - result.joined.len()).min(CONTINUATION_BYTES);
        append(
            &mut result.joined,
            &page[PAGE_HEADER_BYTES..PAGE_HEADER_BYTES + take],
            reader,
        )?;
    }
    if next != 0 {
        return Err(InspectError::Malformed);
    }
    reader.tick()?;
    Ok(result)
}

fn append<R: NativeReadSource>(
    joined: &mut Vec<u8>,
    bytes: &[u8],
    reader: &mut Reader<'_, R>,
) -> Result<(), InspectError> {
    reader.tick()?;
    reader.budget.charge_allocation(bytes.len() as u64, 1)?;
    joined
        .try_reserve(bytes.len())
        .map_err(|_| InspectError::Allocation)?;
    joined.extend_from_slice(bytes);
    Ok(())
}

fn validate(page: &[u8; PAGE_BYTES]) -> Result<(), InspectError> {
    if page[..TABLE_DEFINITION_SIGNATURE.len()] != TABLE_DEFINITION_SIGNATURE {
        return Err(InspectError::Malformed);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
