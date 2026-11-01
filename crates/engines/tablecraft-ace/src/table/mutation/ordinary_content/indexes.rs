//! Complete retained single-I32 membership and page roles.
pub(super) mod declaration;
use super::{
    ledger::{Ledger, Role},
    refused,
};
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, NativeIndex, check_table,
    format::{layout::Layout, page::PAGE_BYTES, row::MASK_BITS},
    header::check,
    inspect_table_indexes,
    limits::MAX_INDEX_DEPTH,
    read_table,
    table::{
        indexes::{
            header::IndexHeader,
            page::{ENTRY_START, LEAF, NODE},
            preserved,
        },
        mutation::{record_roots, rows::RowAssignment},
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const HEADER_BYTES: usize = size_of::<IndexHeader>();
const REFERENCE_BYTES: usize = size_of::<u32>();
const KEY_BYTES_MAX: usize = size_of::<u8>() + size_of::<i32>();
const ENTRY_BYTES_MAX: usize = KEY_BYTES_MAX + 2 * REFERENCE_BYTES;

struct Ordinal(usize);

impl RowAssignment for Ordinal {
    fn row(&self) -> usize {
        self.0
    }
}

pub(super) fn capture(
    image: &NativeImage,
    ledger: &mut Ledger<'_>,
    definition: u32,
    layout: &Layout,
    start: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    if start != layout.index_start {
        return Err(refused());
    }
    let scope = image.catalog().catalog().scope();
    let object = scope.object_id(u64::from(definition));
    let mut name = None;
    for candidate in image.catalog().catalog().objects() {
        check(budget, cancelled)?;
        if candidate.id() == object {
            name = Some(candidate.name().as_str());
            break;
        }
    }
    let name = name.ok_or_else(refused)?;
    let inspection = inspect_table_indexes(&mut image.reader(), scope, name, budget, cancelled)?;
    if inspection.schema().definition_page() != definition
        || inspection.schema().object_id() != object
        || inspection.indexes().len() != layout.logical_indexes
    {
        return Err(refused());
    }
    for index in inspection.indexes() {
        declaration::qualify(inspection.schema(), index, budget, cancelled)?;
    }
    let checked = check_table(&mut image.reader(), scope, name, budget, cancelled)?;
    if checked.schema().definition_page() != definition
        || checked.schema().object_id() != object
        || checked.indexes().len() != layout.logical_indexes
    {
        return Err(refused());
    }
    for index in checked.indexes() {
        check(budget, cancelled)?;
        if index.status() != IndexCheckStatus::Checked {
            return Err(refused());
        }
    }
    let snapshot = read_table(&mut image.reader(), scope, name, budget, cancelled)?;
    if snapshot.schema().object_id() != object
        || snapshot.schema().definition_page() != definition
        || snapshot.rows().len() != checked.rows()
        || snapshot.columns().len() != checked.schema().columns().len()
    {
        return Err(refused());
    }
    let mut ordinals = vector(checked.rows(), budget, cancelled)?;
    for ordinal in 0..checked.rows() {
        check(budget, cancelled)?;
        ordinals.push(Ordinal(ordinal));
    }
    let roots = record_roots::capture(image, checked.schema(), &ordinals, budget, cancelled)?;
    if roots.selected.len() != snapshot.rows().len() {
        return Err(refused());
    }

    let position = declaration::names(
        image,
        checked.schema(),
        checked.indexes().iter().map(|entry| entry.index()),
        budget,
        cancelled,
    )?;

    // Logical aliases are grouped by their real physical data number. Their
    // shared map and pages are claimed exactly once, never as fake collisions.
    for physical in 0..layout.physical_indexes {
        check(budget, cancelled)?;
        let mut selected = None;
        for entry in checked.indexes() {
            check(budget, cancelled)?;
            if entry.index().data_number() as usize != physical {
                continue;
            }
            if let Some(prior) = selected {
                declaration::aliases(prior, entry.index(), budget, cancelled)?;
            } else {
                selected = Some(entry.index());
            }
        }
        let index = selected.ok_or_else(refused)?;
        declaration::physical(
            image,
            checked.schema(),
            index,
            physical,
            checked.indexes().iter().map(|entry| entry.index()),
            budget,
            cancelled,
        )?;

        let key = index.columns()[0];
        let mut population = vector(checked.rows(), budget, cancelled)?;
        for (ordinal, (row, root)) in snapshot.rows().iter().zip(&roots.selected).enumerate() {
            check(budget, cancelled)?;
            if root.update.0 != ordinal {
                return Err(refused());
            }
            let value = match row.get(key.number()) {
                Some(Value::Int32(value)) => Some(*value),
                Some(Value::Null) => None,
                _ => return Err(refused()),
            };
            population.push((root.reference, value));
        }
        let native_owned =
            preserved::certify(image, definition, index, &population, budget, cancelled)?;
        let map = ledger.map(index.usage_map(), budget, cancelled)?;
        let owned = ledger.members(map, budget, cancelled)?;
        budget
            .charge(Resource::Steps, (owned.len() * size_of::<u32>()) as u64)
            .map_err(InspectError::from)?;
        if owned != native_owned {
            return Err(refused());
        }
        for &number in &owned {
            ledger.claim(number, Role::Index, budget, cancelled)?;
        }
        frames(ledger, index, definition, &owned, budget, cancelled)?;
    }
    check(budget, cancelled)?;
    Ok(position)
}

fn vector<T>(
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<T>, EditError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(values)
}

#[derive(Clone, Copy)]
struct Link {
    number: u32,
    next: u32,
}

struct Frames<'work, 'image> {
    ledger: &'work Ledger<'image>,
    owner: u32,
    owned: &'work [u32],
    visited: Vec<u32>,
    levels: Vec<Option<Link>>,
    budget: &'work mut ResourceBudget,
    cancelled: &'work dyn Fn() -> bool,
}

fn frames(
    ledger: &Ledger<'_>,
    index: &NativeIndex,
    owner: u32,
    owned: &[u32],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let visited = vector(owned.len(), budget, cancelled)?;
    let mut levels = vector(MAX_INDEX_DEPTH, budget, cancelled)?;
    levels.resize(MAX_INDEX_DEPTH, None);
    let mut frames = Frames {
        ledger,
        owner,
        owned,
        visited,
        levels,
        budget,
        cancelled,
    };
    frames.node(index.root_page(), 0)?;
    for last in &frames.levels {
        check(frames.budget, cancelled)?;
        if last.is_some_and(|link| link.next != 0) {
            return Err(refused());
        }
    }
    let work = owned
        .len()
        .checked_mul(owned.len().checked_ilog2().unwrap_or(0) as usize + 1)
        .ok_or(InspectError::StructureLimit)?;
    frames
        .budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    frames.visited.sort_unstable();
    frames
        .budget
        .charge(Resource::Steps, size_of_val(owned) as u64)
        .map_err(InspectError::from)?;
    if frames.visited != owned {
        return Err(refused());
    }
    Ok(())
}

impl Frames<'_, '_> {
    fn node(&mut self, number: u32, depth: usize) -> Result<u8, EditError> {
        check(self.budget, self.cancelled)?;
        if depth >= MAX_INDEX_DEPTH || self.visited.len() >= self.owned.len() {
            return Err(refused());
        }
        let work = self
            .visited
            .len()
            .checked_add(self.owned.len().checked_ilog2().unwrap_or(0) as usize + 1)
            .ok_or(InspectError::StructureLimit)?;
        self.budget
            .charge(Resource::Steps, work as u64)
            .map_err(InspectError::from)?;
        if self.owned.binary_search(&number).is_err() || self.visited.contains(&number) {
            return Err(refused());
        }
        self.visited.push(number);
        let page = self.ledger.page(number)?;
        let header = IndexHeader::read(page)?;
        if !matches!(header.kind, LEAF | NODE) || header.owner.get() != self.owner {
            return Err(refused());
        }
        // Native certify already checked the immutable page version, exact key
        // carriers/order/ROW population and graph bounds. This pass closes the
        // remaining reserved fields, levels, all-level links and byte extents.
        let leaf = header.kind == LEAF;
        let maximum = KEY_BYTES_MAX + REFERENCE_BYTES + if leaf { 0 } else { REFERENCE_BYTES };
        let prefix = usize::from(header.prefix.get());
        if prefix > maximum {
            return Err(refused());
        }
        let mut first = [0; ENTRY_BYTES_MAX];
        let mut first_length = None;
        let mut last = 0;
        let mut child_height = None;
        for bit in 0..(ENTRY_START - HEADER_BYTES) * MASK_BITS {
            check(self.budget, self.cancelled)?;
            if page[HEADER_BYTES + bit / MASK_BITS] & (1 << (bit % MASK_BITS)) == 0 {
                continue;
            }
            if bit <= last || bit > PAGE_BYTES - ENTRY_START {
                return Err(refused());
            }
            let shared = if first_length.is_none() { 0 } else { prefix };
            let width = shared
                .checked_add(bit - last)
                .ok_or(InspectError::StructureLimit)?;
            if width > maximum || first_length.is_some_and(|length| shared > length) {
                return Err(refused());
            }
            self.budget
                .charge(Resource::Steps, width as u64)
                .map_err(InspectError::from)?;
            let mut entry = [0; ENTRY_BYTES_MAX];
            entry[..shared].copy_from_slice(&first[..shared]);
            entry[shared..width].copy_from_slice(&page[ENTRY_START + last..ENTRY_START + bit]);
            if first_length.is_none() {
                first = entry;
                first_length = Some(width);
                if prefix > width {
                    return Err(refused());
                }
            }
            if !leaf {
                let child_at = width.checked_sub(REFERENCE_BYTES).ok_or_else(refused)?;
                let child =
                    u32::from_be_bytes(entry[child_at..width].try_into().map_err(|_| refused())?);
                self.child(child, depth + 1, &mut child_height)?;
            }
            last = bit;
        }
        if usize::from(header.free.get()) != PAGE_BYTES - ENTRY_START - last
            || (first_length.is_none() && prefix != 0)
            || (leaf && header.tail.get() != 0)
        {
            return Err(refused());
        }
        if !leaf && header.tail.get() != 0 {
            self.child(header.tail.get(), depth + 1, &mut child_height)?;
        }
        let height = if leaf {
            0
        } else {
            child_height
                .ok_or_else(refused)?
                .checked_add(1)
                .ok_or(InspectError::StructureLimit)?
        };
        if !header.qualified_for_update(height) {
            return Err(refused());
        }
        let prior = self
            .levels
            .get_mut(usize::from(height))
            .ok_or_else(refused)?;
        if let Some(previous) = prior {
            if previous.number != header.previous.get() || previous.next != number {
                return Err(refused());
            }
        } else if header.previous.get() != 0 {
            return Err(refused());
        }
        *prior = Some(Link {
            number,
            next: header.next.get(),
        });
        // [ENTRY_START+last, PAGE_BYTES) is fully classified inactive INDEX
        // capacity. It may retain historical key bytes such as A7. It is never
        // interpreted as a catalog/module/property carrier, and normal updates
        // preserve it through independent whole-image equality.
        Ok(height)
    }

    fn child(
        &mut self,
        number: u32,
        depth: usize,
        previous: &mut Option<u8>,
    ) -> Result<(), EditError> {
        let height = self.node(number, depth)?;
        if previous.is_some_and(|expected| expected != height) {
            return Err(refused());
        }
        *previous = Some(height);
        Ok(())
    }
}
