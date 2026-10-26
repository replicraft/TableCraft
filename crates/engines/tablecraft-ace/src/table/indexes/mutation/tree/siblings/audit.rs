#[cfg(test)]
mod tests;

use super::SiblingAudit;
use crate::{
    EditError, InspectError,
    format::page::PAGE_BYTES,
    header::check,
    table::indexes::{
        header::{IndexHeader, LEVEL_OFFSET},
        mutation::retirement::RetiredPage,
        page::{ENTRY_START, LEAF, NODE},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table) fn audit_siblings(
    before: &[u8],
    after: &[u8],
    audits: &[SiblingAudit],
    retired: &[RetiredPage],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for audit in audits {
        check(budget, cancelled)?;
        if audit.empty_root {
            for &number in &audit.pages {
                if number != audit.root && !audit.retired(number, retired, budget, cancelled)? {
                    return Err(EditError::Preservation);
                }
            }
        }
        for (position, level) in audit.levels.iter().enumerate() {
            let height = u8::try_from(position + 1).map_err(|_| EditError::Preservation)?;
            let mut previous = 0;
            for node in &level.nodes {
                if audit.retired(node.number, retired, budget, cancelled)? {
                    continue;
                }
                check(budget, cancelled)?;
                let original = header(before, node.number, budget, cancelled)?;
                let current = header(after, node.number, budget, cancelled)?;
                let empty_root = audit.empty_root && node.number == audit.root;
                let expected_kind = if empty_root { LEAF } else { NODE };
                let expected_height = if empty_root { 0 } else { height };
                if original.kind != NODE
                    || page(before, node.number)?[LEVEL_OFFSET] != height
                    || current.kind != expected_kind
                    || page(after, node.number)?[LEVEL_OFFSET] != expected_height
                    || current.owner.get() != original.owner.get()
                {
                    return Err(EditError::Preservation);
                }
                if empty_root {
                    let raw = page(after, node.number)?;
                    budget
                        .charge(Resource::Steps, PAGE_BYTES as u64)
                        .map_err(InspectError::from)?;
                    if current.previous.get() != 0
                        || current.next.get() != 0
                        || current.tail.get() != 0
                        || current.prefix.get() != 0
                        || usize::from(current.free.get()) != PAGE_BYTES - ENTRY_START
                        || raw[size_of::<IndexHeader>()..ENTRY_START]
                            .iter()
                            .any(|byte| *byte != 0)
                    {
                        return Err(EditError::Preservation);
                    }
                }
                if level.linked {
                    if current.previous.get() != previous {
                        return Err(EditError::Preservation);
                    }
                    if previous != 0
                        && header(after, previous, budget, cancelled)?.next.get() != node.number
                    {
                        return Err(EditError::Preservation);
                    }
                    previous = node.number;
                } else if current.previous.get() != 0 || current.next.get() != 0 {
                    return Err(EditError::Preservation);
                }
            }
            if previous != 0 && header(after, previous, budget, cancelled)?.next.get() != 0 {
                return Err(EditError::Preservation);
            }
        }
    }
    Ok(())
}

fn page(bytes: &[u8], number: u32) -> Result<&[u8], EditError> {
    let start = (number as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    let end = start
        .checked_add(PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    bytes.get(start..end).ok_or(EditError::Preservation)
}

fn header<'a>(
    bytes: &'a [u8],
    number: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a IndexHeader, EditError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, size_of::<IndexHeader>() as u64)
        .map_err(InspectError::from)?;
    Ok(IndexHeader::read(page(bytes, number)?)?)
}
