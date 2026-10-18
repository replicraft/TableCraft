//! Bounded logical table-definition bytes, excluding continuation page headers.
use super::{
    bytes::u32_at,
    page::{PAGE_BYTES, TABLE_DEFINITION_SIGNATURE},
    reader::Reader,
};
use crate::{InspectError, profile::MAX_DEFINITION_PAGES};
use std::{
    io::{Read, Seek},
    ops::Deref,
};

const PAGE_HEADER_BYTES: usize = 2 * size_of::<u16>() + size_of::<u32>();
const NEXT_PAGE_OFFSET: usize = PAGE_HEADER_BYTES - size_of::<u32>();
const LENGTH_OFFSET: usize = PAGE_HEADER_BYTES;
const MIN_DEFINITION_BYTES: usize = LENGTH_OFFSET + size_of::<u32>();
const CONTINUATION_BYTES: usize = PAGE_BYTES - PAGE_HEADER_BYTES;
const MAX_DEFINITION_BYTES: usize = PAGE_BYTES + (MAX_DEFINITION_PAGES - 1) * CONTINUATION_BYTES;

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
pub(crate) fn read<R: Read + Seek>(
    reader: &mut Reader<'_, R>,
    root: u32,
) -> Result<Definition, InspectError> {
    let first = reader.page(root)?;
    validate(&first)?;
    let length = u32_at(&first, LENGTH_OFFSET)? as usize;
    if length < MIN_DEFINITION_BYTES {
        return Err(InspectError::Malformed);
    }
    if length > MAX_DEFINITION_BYTES {
        return Err(InspectError::StructureLimit);
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
        return Ok(result);
    }
    reader.budget.charge_allocation(length as u64, 1)?;
    result
        .joined
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    result.joined.extend_from_slice(&result.first);
    let mut visited = [0_u32; MAX_DEFINITION_PAGES];
    visited[0] = root;
    let mut count = 1;
    while result.joined.len() < length {
        reader.tick()?;
        if next == 0 {
            return Err(InspectError::Truncated);
        }
        if visited[..count].contains(&next) {
            return Err(InspectError::Malformed);
        }
        if count == visited.len() {
            return Err(InspectError::StructureLimit);
        }
        visited[count] = next;
        count += 1;
        let page = reader.page(next)?;
        validate(&page)?;
        next = u32_at(&page, NEXT_PAGE_OFFSET)?;
        let take = (length - result.joined.len()).min(CONTINUATION_BYTES);
        result
            .joined
            .extend_from_slice(&page[PAGE_HEADER_BYTES..PAGE_HEADER_BYTES + take]);
    }
    if next != 0 {
        return Err(InspectError::Malformed);
    }
    reader.tick()?;
    Ok(result)
}
fn validate(page: &[u8; PAGE_BYTES]) -> Result<(), InspectError> {
    if page[..TABLE_DEFINITION_SIGNATURE.len()] != TABLE_DEFINITION_SIGNATURE {
        return Err(InspectError::Malformed);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
