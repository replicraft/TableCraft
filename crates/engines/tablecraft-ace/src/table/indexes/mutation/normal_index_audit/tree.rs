//! Original-only compressed page graph, partitions, bounds and sibling facts.
#[cfg(feature = "journal")]
mod binary;
#[cfg(feature = "journal")]
mod text;
use super::{atoms, vector};
use crate::{
    EditError, InspectError, NativeColumn, NativeImage, NativeIndex, WriteBlocker,
    format::{
        allocation,
        data_type::{CURRENCY, GUID, INT64},
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    limits::MAX_INDEX_DEPTH,
    table::indexes::{
        header::IndexHeader,
        page::{ENTRY_START, LEAF, NODE, PAGE_VERSION},
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Guid,
};

const MASK_START: usize = size_of::<IndexHeader>();
const GUID_COMPONENT_BYTES: usize = size_of::<u8>() + Guid::BYTE_LEN + 2 * size_of::<u8>();
const REFERENCE_BYTES: usize = size_of::<u32>();

pub(super) struct Node {
    pub number: u32,
    pub prefix: usize,
    pub entries: Vec<Vec<u8>>,
    pub children: Vec<Node>,
    pub height: u8,
}

pub(in crate::table) struct Tree {
    pub(super) root: Node,
    pub(super) pages: Vec<u32>,
}

struct Link {
    number: u32,
    previous: u32,
    next: u32,
}

#[derive(Clone, Copy)]
enum Component {
    Integral(usize),
    Guid,
}

impl Component {
    fn bytes(self) -> usize {
        match self {
            Self::Integral(width) => size_of::<u8>() + width,
            Self::Guid => GUID_COMPONENT_BYTES,
        }
    }
}

struct Capture<'image, 'index, 'budget> {
    image: &'image NativeImage,
    index: &'index NativeIndex,
    owner: u32,
    component: Component,
    retained_integral: bool,
    owned: &'index [u32],
    visited: Vec<u32>,
    levels: Vec<Vec<Link>>,
    budget: &'budget mut ResourceBudget,
    cancelled: &'budget dyn Fn() -> bool,
}

impl Tree {
    pub(super) fn capture(
        image: &NativeImage,
        index: &NativeIndex,
        owner: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_width(
            image,
            index,
            owner,
            size_of::<i32>(),
            false,
            budget,
            cancelled,
        )
    }

    pub(super) fn capture_guid(
        image: &NativeImage,
        index: &NativeIndex,
        owner: u32,
        field: &NativeColumn,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        if field.native_type() != GUID
            || field.scalar_type() != Some(tablecraft_schema::ScalarType::Guid)
            || !field.fixed()
            || field.storage_length() != Guid::BYTE_LEN
            || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
            || field.extended_flags() != 0
            || field.complex_type_id().is_some()
            || index.columns().len() != 1
            || index.columns()[0].number() != field.number()
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let raw = reader.page(index.root_page())?;
        reader.work(size_of::<IndexHeader>())?;
        let header = IndexHeader::read(&raw)?;
        if header.kind != LEAF
            || !header.qualified_for_update(0)
            || header.previous.get() != 0
            || header.next.get() != 0
            || header.tail.get() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let tree = Self::capture_component(
            image,
            index,
            owner,
            Component::Guid,
            false,
            budget,
            cancelled,
        )?;
        if !tree.root.children.is_empty() || tree.pages != [index.root_page()] {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        Ok(tree)
    }

    pub(super) fn capture_integral(
        image: &NativeImage,
        index: &NativeIndex,
        owner: u32,
        field: &NativeColumn,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_integral_with(image, index, owner, field, false, budget, cancelled)
    }

    pub(super) fn capture_retained_integral(
        image: &NativeImage,
        index: &NativeIndex,
        owner: u32,
        field: &NativeColumn,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_integral_with(image, index, owner, field, true, budget, cancelled)
    }

    fn capture_integral_with(
        image: &NativeImage,
        index: &NativeIndex,
        owner: u32,
        field: &NativeColumn,
        retained_integral: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        if !matches!(field.native_type(), INT64 | CURRENCY)
            || !field.fixed()
            || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
            || field.extended_flags() != 0
            || field.storage_length() != size_of::<i64>()
            || field.complex_type_id().is_some()
            || index.columns().len() != 1
            || index.columns()[0].number() != field.number()
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        Self::capture_width(
            image,
            index,
            owner,
            size_of::<i64>(),
            retained_integral,
            budget,
            cancelled,
        )
    }

    fn capture_width(
        image: &NativeImage,
        index: &NativeIndex,
        owner: u32,
        value_bytes: usize,
        retained_integral: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_component(
            image,
            index,
            owner,
            Component::Integral(value_bytes),
            retained_integral,
            budget,
            cancelled,
        )
    }

    fn capture_component(
        image: &NativeImage,
        index: &NativeIndex,
        owner: u32,
        component: Component,
        retained_integral: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let mut source = image.reader();
        let owned = allocation::pages(
            &mut Reader {
                source: &mut source,
                header: image.catalog().header(),
                budget,
                cancelled,
                reads: 0,
            },
            index.usage_map(),
        )?;
        let mut levels = vector(MAX_INDEX_DEPTH, budget, cancelled)?;
        for _ in 0..MAX_INDEX_DEPTH {
            levels.push(Vec::new());
        }
        let visited = vector(owned.len(), budget, cancelled)?;
        let mut capture = Capture {
            image,
            index,
            owner,
            component,
            retained_integral,
            owned: &owned,
            visited,
            levels,
            budget,
            cancelled,
        };
        let root = capture.node(index.root_page(), 0)?;
        capture.links()?;
        let work = capture
            .visited
            .len()
            .checked_mul(capture.visited.len().checked_ilog2().unwrap_or(0) as usize + 1)
            .ok_or(InspectError::StructureLimit)?;
        capture
            .budget
            .charge(Resource::Steps, work as u64)
            .map_err(InspectError::from)?;
        capture.visited.sort_unstable();
        capture
            .budget
            .charge(Resource::Steps, (owned.len() * size_of::<u32>()) as u64)
            .map_err(InspectError::from)?;
        if capture.visited != owned {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        check(capture.budget, cancelled)?;
        Ok(Self {
            root,
            pages: capture.visited,
        })
    }

    /// Verify leaf membership and all original separator bounds. Original
    /// upper bounds are not required to equal an actual child maximum.
    pub(in crate::table) fn original(
        &self,
        keys: &[Vec<u8>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let mut position = 0;
        self.root
            .original(keys, &mut position, None, None, budget, cancelled)?;
        if position != keys.len() {
            return Err(EditError::Preservation);
        }
        Ok(())
    }
}

impl Capture<'_, '_, '_> {
    fn node(&mut self, number: u32, depth: usize) -> Result<Node, EditError> {
        check(self.budget, self.cancelled)?;
        if depth >= MAX_INDEX_DEPTH {
            return Err(InspectError::StructureLimit.into());
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
            return Err(InspectError::Malformed.into());
        }
        self.visited.push(number);
        let mut source = self.image.reader();
        let raw = Reader {
            source: &mut source,
            header: self.image.catalog().header(),
            budget: &mut *self.budget,
            cancelled: self.cancelled,
            reads: 0,
        }
        .page(number)?;
        let header = IndexHeader::read(&raw)?;
        if !matches!(header.kind, LEAF | NODE)
            || header.version != PAGE_VERSION
            || header.owner.get() != self.owner
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let leaf = header.kind == LEAF;
        let prefix = usize::from(header.prefix.get());
        if matches!(self.component, Component::Integral(width) if width == size_of::<i64>())
            && !self.retained_integral
            && (!leaf || depth != 0 || prefix != 0)
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let key_bytes = self
            .index
            .columns()
            .len()
            .checked_mul(self.component.bytes())
            .and_then(|bytes| bytes.checked_add(REFERENCE_BYTES))
            .ok_or(InspectError::StructureLimit)?;
        let maximum_width = key_bytes
            .checked_add(if leaf { 0 } else { REFERENCE_BYTES })
            .ok_or(InspectError::StructureLimit)?;
        if prefix > maximum_width {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let count = raw[MASK_START..ENTRY_START]
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum();
        self.budget
            .charge(Resource::Steps, (ENTRY_START - MASK_START) as u64)
            .map_err(InspectError::from)?;
        let mut entries: Vec<Vec<u8>> = vector(count, self.budget, self.cancelled)?;
        let mut last = 0;
        let mut previous_key_end = 0_usize;
        for bit in 0..(ENTRY_START - MASK_START) * u8::BITS as usize {
            check(self.budget, self.cancelled)?;
            if raw[MASK_START + bit / u8::BITS as usize] & (1 << (bit % u8::BITS as usize)) == 0 {
                continue;
            }
            let shared = if entries.is_empty() { 0 } else { prefix };
            let width = shared
                .checked_add(bit.checked_sub(last).ok_or(EditError::Preservation)?)
                .filter(|width| *width <= maximum_width)
                .ok_or(EditError::Preservation)?;
            if bit <= last
                || ENTRY_START
                    .checked_add(bit)
                    .is_none_or(|end| end > PAGE_BYTES)
            {
                return Err(EditError::Preservation);
            }
            let mut entry = vector(width, self.budget, self.cancelled)?;
            self.budget
                .charge(Resource::Steps, width as u64)
                .map_err(InspectError::from)?;
            if let Some(first) = entries.first() {
                entry.extend_from_slice(first.get(..shared).ok_or(EditError::Preservation)?);
            }
            entry.extend_from_slice(&raw[ENTRY_START + last..ENTRY_START + bit]);
            let facts = match self.component {
                Component::Integral(width) if width == size_of::<i32>() => {
                    atoms::inspect(&entry, self.index, leaf, self.budget, self.cancelled)?
                }
                Component::Integral(width) => atoms::inspect_width(
                    &entry,
                    self.index,
                    leaf,
                    width,
                    self.budget,
                    self.cancelled,
                )?,
                Component::Guid => {
                    atoms::inspect_guid(&entry, self.index, leaf, self.budget, self.cancelled)?
                }
            };
            if let Some(previous) = entries.last() {
                self.budget
                    .charge(Resource::Steps, (previous_key_end + facts.key_end) as u64)
                    .map_err(InspectError::from)?;
                if previous[..previous_key_end] >= entry[..facts.key_end] {
                    return Err(InspectError::Malformed.into());
                }
            }
            previous_key_end = facts.key_end;
            entries.push(entry);
            last = bit;
        }
        if (entries.is_empty() && prefix != 0)
            || usize::from(header.free.get()) != PAGE_BYTES - ENTRY_START - last
            || leaf && header.tail.get() != 0
            || leaf && depth != 0 && entries.is_empty()
            || !leaf && entries.is_empty() && header.tail.get() == 0
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let child_count = if leaf {
            0
        } else {
            entries.len() + usize::from(header.tail.get() != 0)
        };
        let mut children = vector(child_count, self.budget, self.cancelled)?;
        if !leaf {
            for entry in &entries {
                check(self.budget, self.cancelled)?;
                let child = u32::from_be_bytes(
                    entry[entry
                        .len()
                        .checked_sub(REFERENCE_BYTES)
                        .ok_or(EditError::Preservation)?..]
                        .try_into()
                        .map_err(|_| EditError::Preservation)?,
                );
                children.push(self.node(child, depth + 1)?);
            }
            if header.tail.get() != 0 {
                children.push(self.node(header.tail.get(), depth + 1)?);
            }
        }
        let height = if leaf {
            0
        } else {
            let child_height = children.first().ok_or(EditError::Preservation)?.height;
            if children.iter().any(|child| child.height != child_height) {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            child_height
                .checked_add(1)
                .ok_or(InspectError::StructureLimit)?
        };
        if !header.qualified_for_update(height) {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let level = self
            .levels
            .get_mut(usize::from(height))
            .ok_or(InspectError::StructureLimit)?;
        self.budget
            .charge_allocation(1, size_of::<Link>() as u64)
            .map_err(InspectError::from)?;
        level
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        level.push(Link {
            number,
            previous: header.previous.get(),
            next: header.next.get(),
        });
        Ok(Node {
            number,
            prefix,
            entries,
            children,
            height,
        })
    }

    fn links(&mut self) -> Result<(), EditError> {
        for (height, level) in self.levels.iter().enumerate() {
            check(self.budget, self.cancelled)?;
            self.budget
                .charge(Resource::Steps, level.len() as u64)
                .map_err(InspectError::from)?;
            let linked = height == 0
                || level
                    .iter()
                    .any(|link| link.previous != 0 || link.next != 0);
            if !linked {
                continue;
            }
            for (position, link) in level.iter().enumerate() {
                check(self.budget, self.cancelled)?;
                let previous = position
                    .checked_sub(1)
                    .map_or(0, |index| level[index].number);
                let next = level.get(position + 1).map_or(0, |link| link.number);
                if link.previous != previous || link.next != next {
                    return Err(InspectError::Malformed.into());
                }
            }
        }
        Ok(())
    }
}

impl Node {
    fn original(
        &self,
        keys: &[Vec<u8>],
        position: &mut usize,
        lower: Option<&[u8]>,
        upper: Option<&[u8]>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        let mut previous = lower;
        for entry in &self.entries {
            check(budget, cancelled)?;
            let width = entry
                .len()
                .checked_sub(if self.children.is_empty() {
                    0
                } else {
                    REFERENCE_BYTES
                })
                .ok_or(EditError::Preservation)?;
            let key = &entry[..width];
            budget
                .charge(Resource::Steps, (2 * width) as u64)
                .map_err(InspectError::from)?;
            if previous.is_some_and(|lower| key <= lower) || upper.is_some_and(|upper| key > upper)
            {
                return Err(EditError::Preservation);
            }
            previous = Some(key);
        }
        if self.children.is_empty() {
            for entry in &self.entries {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, (entry.len() + size_of::<Vec<u8>>()) as u64)
                    .map_err(InspectError::from)?;
                if keys.get(*position) != Some(entry) {
                    return Err(EditError::Preservation);
                }
                *position = position
                    .checked_add(1)
                    .ok_or(InspectError::StructureLimit)?;
            }
        } else {
            let mut previous = lower;
            for (ordinal, child) in self.children.iter().enumerate() {
                check(budget, cancelled)?;
                let bound = self
                    .entries
                    .get(ordinal)
                    .map(|entry| &entry[..entry.len() - REFERENCE_BYTES]);
                child.original(keys, position, previous, bound.or(upper), budget, cancelled)?;
                if bound.is_some() {
                    previous = bound;
                }
            }
        }
        Ok(())
    }
}

impl Tree {
    pub(in crate::table) fn pages(&self) -> &[u32] {
        &self.pages
    }
}
