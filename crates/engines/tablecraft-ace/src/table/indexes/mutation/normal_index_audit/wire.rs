//! Manual Int32 keys and source-partitioned fitting page reconstruction.
use super::{
    atoms,
    population::Row,
    tree::{Node, Tree},
    vector,
};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableCheck, WriteBlocker,
    format::page::PAGE_BYTES,
    header::check,
    table::indexes::{
        header::IndexHeader,
        key::{ASC_NULL, ASC_VALUE, DESC_NULL, DESC_VALUE, INT32_SIGN_BIT},
        page::ENTRY_START,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

#[cfg(test)]
mod tests;

const FREE_OFFSET: usize = std::mem::offset_of!(IndexHeader, free);
const PREFIX_OFFSET: usize = std::mem::offset_of!(IndexHeader, prefix);
const MASK_START: usize = size_of::<IndexHeader>();
const MASK_BYTES: usize = ENTRY_START - MASK_START;
const COMPONENT_BYTES: usize = size_of::<u8>() + size_of::<i32>();
const REFERENCE_BYTES: usize = size_of::<u32>();

#[derive(Clone, Copy)]
pub(super) enum Columns<'a> {
    Complete,
    Keys(&'a [usize]),
}

pub(super) enum Population<'a> {
    Present(&'a [Row]),
    Nullable(&'a [Row<Option<i32>>]),
}

trait Cell {
    fn value(&self) -> Option<i32>;
}

impl Cell for i32 {
    fn value(&self) -> Option<i32> {
        Some(*self)
    }
}

impl Cell for Option<i32> {
    fn value(&self) -> Option<i32> {
        *self
    }
}

pub(super) struct Operation<'a> {
    pub image: &'a NativeImage,
    pub checked: &'a TableCheck,
    pub index: &'a NativeIndex,
    pub population: Population<'a>,
    pub columns: Columns<'a>,
    pub tree: &'a Tree,
}

fn keys<T: Cell>(
    population: &[Row<T>],
    columns: &[(usize, bool)],
    after: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<u8>>, EditError> {
    let width = columns
        .len()
        .checked_mul(COMPONENT_BYTES)
        .and_then(|width| width.checked_add(REFERENCE_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    let mut keys = vector(population.len(), budget, cancelled)?;
    for row in population {
        check(budget, cancelled)?;
        let values = if after { &row.after } else { &row.before };
        let mut wire = vector(width, budget, cancelled)?;
        for &(position, descending) in columns {
            check(budget, cancelled)?;
            let value = values.get(position).ok_or(EditError::Preservation)?.value();
            match value {
                None => wire.push(if descending { DESC_NULL } else { ASC_NULL }),
                Some(value) => {
                    let mut bytes = ((value as u32) ^ INT32_SIGN_BIT).to_be_bytes();
                    if descending {
                        for byte in &mut bytes {
                            *byte = !*byte;
                        }
                    }
                    wire.push(if descending { DESC_VALUE } else { ASC_VALUE });
                    wire.extend_from_slice(&bytes);
                }
            }
        }
        wire.extend_from_slice(&row.reference.to_be_bytes());
        if wire.len() > width {
            return Err(EditError::Preservation);
        }
        keys.push(wire);
    }
    let work = keys
        .len()
        .checked_mul(keys.len().checked_ilog2().unwrap_or(0) as usize + 1)
        .and_then(|count| count.checked_mul(width))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    keys.sort_unstable();
    check(budget, cancelled)?;
    Ok(keys)
}

fn population_keys(
    population: &Population<'_>,
    columns: &[(usize, bool)],
    after: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<u8>>, EditError> {
    match population {
        Population::Present(rows) => keys(rows, columns, after, budget, cancelled),
        Population::Nullable(rows) => keys(rows, columns, after, budget, cancelled),
    }
}

struct Layout {
    prefix: usize,
    bytes: usize,
    bitmap: [u8; MASK_BYTES],
}

fn layout(
    entries: &[Vec<u8>],
    node: &Node,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Layout, EditError> {
    let mut total = 0_usize;
    let mut common = 0;
    for (position, entry) in entries.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, entry.len() as u64)
            .map_err(InspectError::from)?;
        total = total
            .checked_add(entry.len())
            .ok_or(InspectError::StructureLimit)?;
        let key_bytes = entry
            .len()
            .checked_sub(if node.children.is_empty() {
                0
            } else {
                REFERENCE_BYTES
            })
            .ok_or(EditError::Preservation)?;
        if position == 0 {
            common = if entries.len() == 1 {
                entry.len()
            } else {
                key_bytes
            };
        } else {
            common = common.min(key_bytes);
            common = entries[0][..common]
                .iter()
                .zip(&entry[..common])
                .take_while(|(left, right)| left == right)
                .count();
        }
    }

    let saved = entries.len().saturating_sub(1);
    let mut prefix = node.prefix.min(common);
    let retained = total
        .checked_sub(prefix.checked_mul(saved).ok_or(EditError::Preservation)?)
        .ok_or(EditError::Preservation)?;
    if retained > PAGE_BYTES - ENTRY_START {
        prefix = common;
    }
    let bytes = total
        .checked_sub(prefix.checked_mul(saved).ok_or(EditError::Preservation)?)
        .filter(|bytes| *bytes <= PAGE_BYTES - ENTRY_START)
        .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
    let mut bitmap = [0; MASK_BYTES];
    let mut end = 0_usize;
    for (position, entry) in entries.iter().enumerate() {
        check(budget, cancelled)?;
        let shared = if position == 0 { 0 } else { prefix };
        end = end
            .checked_add(
                entry
                    .len()
                    .checked_sub(shared)
                    .ok_or(EditError::Preservation)?,
            )
            .ok_or(EditError::Preservation)?;
        let bit = bitmap
            .get_mut(end / u8::BITS as usize)
            .ok_or(EditError::Preservation)?;
        *bit |= 1 << (end % u8::BITS as usize);
    }
    if end != bytes {
        return Err(EditError::Preservation);
    }
    Ok(Layout {
        prefix,
        bytes,
        bitmap,
    })
}

struct Replay<'a, 'budget> {
    image: &'a NativeImage,
    keys: &'a [Vec<u8>],
    expected: &'a mut [u8],
    position: usize,
    budget: &'budget mut ResourceBudget,
    cancelled: &'budget dyn Fn() -> bool,
}

impl Replay<'_, '_> {
    fn node(&mut self, node: &Node) -> Result<Option<usize>, EditError> {
        check(self.budget, self.cancelled)?;
        let mut entries = vector(node.entries.len(), self.budget, self.cancelled)?;
        let maximum = if node.children.is_empty() {
            let start = self.position;
            let end = start
                .checked_add(node.entries.len())
                .filter(|end| *end <= self.keys.len())
                .ok_or(EditError::Preservation)?;
            for key in &self.keys[start..end] {
                let mut entry = vector(key.len(), self.budget, self.cancelled)?;
                self.budget
                    .charge(Resource::Steps, key.len() as u64)
                    .map_err(InspectError::from)?;
                entry.extend_from_slice(key);
                entries.push(entry);
            }
            self.position = end;
            (start != end).then(|| end - 1)
        } else {
            let mut maximum = None;
            for (position, child) in node.children.iter().enumerate() {
                check(self.budget, self.cancelled)?;
                let last = self.node(child)?.ok_or(EditError::Preservation)?;
                if position < node.entries.len() {
                    let key = self.keys.get(last).ok_or(EditError::Preservation)?;
                    let width = key
                        .len()
                        .checked_add(REFERENCE_BYTES)
                        .ok_or(InspectError::StructureLimit)?;
                    let mut entry = vector(width, self.budget, self.cancelled)?;
                    self.budget
                        .charge(Resource::Steps, width as u64)
                        .map_err(InspectError::from)?;
                    entry.extend_from_slice(key);
                    entry.extend_from_slice(&child.number.to_be_bytes());
                    entries.push(entry);
                }
                maximum = Some(last);
            }
            maximum
        };
        if entries.len() != node.entries.len() {
            return Err(EditError::Preservation);
        }

        let layout = layout(&entries, node, self.budget, self.cancelled)?;
        let start = (node.number as usize)
            .checked_mul(PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        let end = start
            .checked_add(PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        let original = self
            .image
            .as_bytes()
            .get(start..end)
            .ok_or(EditError::Preservation)?;
        let page = self
            .expected
            .get_mut(start..end)
            .ok_or(EditError::Preservation)?;
        self.budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        if page != original {
            return Err(EditError::Preservation);
        }
        let free = u16::try_from(PAGE_BYTES - ENTRY_START - layout.bytes)
            .map_err(|_| EditError::Preservation)?;
        let prefix = u16::try_from(layout.prefix).map_err(|_| EditError::Preservation)?;
        page[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()].copy_from_slice(&free.to_le_bytes());
        page[PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()]
            .copy_from_slice(&prefix.to_le_bytes());
        self.budget
            .charge(Resource::Steps, MASK_BYTES as u64)
            .map_err(InspectError::from)?;
        page[MASK_START..ENTRY_START].copy_from_slice(&layout.bitmap);
        let mut position = ENTRY_START;
        for (ordinal, entry) in entries.iter().enumerate() {
            check(self.budget, self.cancelled)?;
            let shared = if ordinal == 0 { 0 } else { layout.prefix };
            let suffix = entry.get(shared..).ok_or(EditError::Preservation)?;
            self.budget
                .charge(Resource::Steps, suffix.len() as u64)
                .map_err(InspectError::from)?;
            let end = position
                .checked_add(suffix.len())
                .filter(|end| *end <= ENTRY_START + layout.bytes)
                .ok_or(EditError::Preservation)?;
            page.get_mut(position..end)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(suffix);
            position = end;
        }
        if position != ENTRY_START + layout.bytes {
            return Err(EditError::Preservation);
        }
        // Vacated bytes retain the original page. Header, topology, child/tail
        // identities and the complete unused complement are not reconstructed.
        check(self.budget, self.cancelled)?;
        Ok(maximum)
    }
}

pub(super) fn reconstruct(
    operation: &Operation<'_>,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let schema = operation.checked.schema();
    let mut columns = vector(operation.index.columns().len(), budget, cancelled)?;
    for key in operation.index.columns() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, schema.columns().len() as u64)
            .map_err(InspectError::from)?;
        let position = schema
            .columns()
            .iter()
            .position(|column| column.number() == key.number())
            .ok_or(InspectError::Malformed)?;
        let dense = match operation.columns {
            Columns::Complete => position,
            Columns::Keys(positions) => {
                budget
                    .charge(Resource::Steps, positions.len() as u64)
                    .map_err(InspectError::from)?;
                positions
                    .iter()
                    .position(|native| *native == position)
                    .ok_or(EditError::Preservation)?
            }
        };
        columns.push((dense, key.descending()));
    }
    let original = population_keys(&operation.population, &columns, false, budget, cancelled)?;
    let final_keys = population_keys(&operation.population, &columns, true, budget, cancelled)?;
    operation.tree.original(&original, budget, cancelled)?;
    for alias in operation.checked.indexes() {
        check(budget, cancelled)?;
        if alias.index().data_number() != operation.index.data_number() {
            continue;
        }
        let constraints = alias
            .index()
            .constraints()
            .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        let mut previous = None;
        for key in &final_keys {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, key.len() as u64)
                .map_err(InspectError::from)?;
            let facts = atoms::inspect(key, operation.index, true, budget, cancelled)?;
            let value = &key[..key
                .len()
                .checked_sub(REFERENCE_BYTES)
                .ok_or(EditError::Preservation)?];
            constraints
                .check_tuple(
                    &mut previous,
                    if facts.all_null { None } else { Some(value) },
                    facts.any_null,
                )
                .map_err(InspectError::from)?;
        }
    }
    let mut work = original
        .len()
        .checked_mul(size_of::<Vec<u8>>())
        .ok_or(InspectError::StructureLimit)?;
    for key in &original {
        work = work
            .checked_add(key.len())
            .ok_or(InspectError::StructureLimit)?;
    }
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    if original == final_keys {
        // Preserve all retained original upper bounds when the whole physical
        // index is unchanged. Affected indexes alone select fitting repacking.
        for page in &operation.tree.pages {
            check(budget, cancelled)?;
            let start = (*page as usize)
                .checked_mul(PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            let end = start
                .checked_add(PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            budget
                .charge(Resource::Steps, PAGE_BYTES as u64)
                .map_err(InspectError::from)?;
            if expected.get(start..end) != operation.image.as_bytes().get(start..end) {
                return Err(EditError::Preservation);
            }
        }
        check(budget, cancelled)?;
        return Ok(());
    }
    let mut replay = Replay {
        image: operation.image,
        keys: &final_keys,
        expected,
        position: 0,
        budget,
        cancelled,
    };
    replay.node(&operation.tree.root)?;
    if replay.position != final_keys.len() {
        return Err(EditError::Preservation);
    }
    check(replay.budget, cancelled)?;
    Ok(())
}
