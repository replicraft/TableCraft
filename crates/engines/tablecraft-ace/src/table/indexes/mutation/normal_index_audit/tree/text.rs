//! Complete bounded Text root-leaf ownership; no mutation or key production.
use super::{MASK_START, Node, Tree, vector};
use crate::{
    EditError, InspectError, NativeColumn, NativeImage, NativeIndex, WriteBlocker,
    format::{
        allocation,
        data_type::{MAX_SHORT_TEXT_UNITS, SHORT_TEXT},
        layout::{INDEX_KEY_BYTES, UPDATABLE_FLAG},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    table::indexes::{
        header::IndexHeader,
        page::{ENTRY_START, LEAF, PAGE_VERSION},
    },
};
use std::cmp::Ordering;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

const ROW_BYTES: usize = size_of::<u32>();
const NULL_BYTES: usize = size_of::<u8>();
const ASCENDING_NULL: u8 = 0;
const ASCENDING_PRESENT: u8 = 0x7f;
const MIN_PRESENT_BYTES: usize = 3;

impl Tree {
    pub(in crate::table::indexes::mutation::normal_index_audit) fn capture_text(
        image: &NativeImage,
        index: &NativeIndex,
        owner: u32,
        field: &NativeColumn,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        if field.native_type() != SHORT_TEXT
            || field.scalar_type() != Some(ScalarType::Text)
            || field.fixed()
            || field.flags() != UPDATABLE_FLAG
            || field.extended_flags() != 0
            || field.complex_type_id().is_some()
            || field.storage_length() == 0
            || !field.storage_length().is_multiple_of(size_of::<u16>())
            || field.storage_length() / size_of::<u16>() > MAX_SHORT_TEXT_UNITS
            || !field
                .text_sort_order()
                .is_some_and(|order| order.general_family().is_some())
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
        let owned = allocation::pages(&mut reader, index.usage_map())?;
        reader.work(owned.len())?;
        if owned != [index.root_page()] {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let page = reader.page(index.root_page())?;
        let header = IndexHeader::read(&page)?;
        if header.kind != LEAF
            || header.version != PAGE_VERSION
            || header.owner.get() != owner
            || !header.qualified_for_update(0)
            || header.previous.get() != 0
            || header.next.get() != 0
            || header.tail.get() != 0
            || header.prefix.get() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        reader.work(ENTRY_START - MASK_START)?;
        let count = page[MASK_START..ENTRY_START]
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum();
        let mut entries: Vec<Vec<u8>> = vector(count, reader.budget, cancelled)?;
        let mut end = 0_usize;
        for bit in 0..(ENTRY_START - MASK_START) * u8::BITS as usize {
            reader.tick()?;
            if page[MASK_START + bit / u8::BITS as usize] & (1 << (bit % u8::BITS as usize)) == 0 {
                continue;
            }
            if bit <= end || ENTRY_START + bit > PAGE_BYTES {
                return Err(EditError::Preservation);
            }
            let raw = &page[ENTRY_START + end..ENTRY_START + bit];
            component(
                raw,
                field.storage_length(),
                index.columns()[0].descending(),
                reader.budget,
                cancelled,
            )?;
            if let Some(previous) = entries.last() {
                reader.work(previous.len() + raw.len())?;
                if compare(previous, raw)? != Ordering::Less {
                    return Err(InspectError::Malformed.into());
                }
            }
            let mut entry = vector(raw.len(), reader.budget, cancelled)?;
            reader.work(raw.len())?;
            entry.extend_from_slice(raw);
            entries.push(entry);
            end = bit;
        }
        if usize::from(header.free.get()) != PAGE_BYTES - ENTRY_START - end {
            return Err(EditError::Preservation);
        }
        check(reader.budget, cancelled)?;
        Ok(Self {
            root: Node {
                number: index.root_page(),
                prefix: 0,
                entries,
                children: Vec::new(),
                height: 0,
            },
            pages: owned,
        })
    }
}

// Text payload interpretation has already passed the actual complete native
// checker. Ownership independently closes root framing, marker/ROW width and
// byte ordering; it does not infer source membership from this opaque carrier.
fn component(
    entry: &[u8],
    _maximum: usize,
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let end = entry
        .len()
        .checked_sub(ROW_BYTES)
        .ok_or(EditError::Preservation)?;
    let wire = entry.get(..end).ok_or(EditError::Preservation)?;
    let marker = *wire.first().ok_or(EditError::Preservation)?;
    let null = if descending {
        !ASCENDING_NULL
    } else {
        ASCENDING_NULL
    };
    let present = if descending {
        !ASCENDING_PRESENT
    } else {
        ASCENDING_PRESENT
    };
    if marker == null {
        if wire.len() != NULL_BYTES {
            return Err(EditError::Preservation);
        }
    } else if marker != present || !(MIN_PRESENT_BYTES..=INDEX_KEY_BYTES).contains(&wire.len()) {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, entry.len() as u64)
        .map_err(InspectError::from)?;
    check(budget, cancelled)?;
    Ok(())
}

fn compare(left: &[u8], right: &[u8]) -> Result<Ordering, EditError> {
    let left_end = left
        .len()
        .checked_sub(ROW_BYTES)
        .ok_or(EditError::Preservation)?;
    let right_end = right
        .len()
        .checked_sub(ROW_BYTES)
        .ok_or(EditError::Preservation)?;
    Ok(left[..left_end]
        .cmp(&right[..right_end])
        .then_with(|| left[left_end..].cmp(&right[right_end..])))
}
