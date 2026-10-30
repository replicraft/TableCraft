use super::{
    SecondaryIndexCreate, buffer,
    layout::{self, CreatedDescriptor},
    source::Original,
};
use crate::{
    EditError, InspectError, NativeImage,
    format::{
        allocation::{
            GLOBAL_FREE_REFERENCE, INLINE_BITMAP_OFFSET, INLINE_MAP, MapRole,
            QualifiedInlineRecord, RepresentedMap,
        },
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    table::{
        indexes::mutation::create::{CreatedPage, Population},
        mutation::append::{State, owners::fresh_index::FreshIndexOwner},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const DATA_KIND: u8 = 1;
const DATA_VERSION: u8 = 1;
const DATA_ROW_COUNT_OFFSET: usize = 12;
const DATA_SLOT_OFFSET: usize = DATA_ROW_COUNT_OFFSET + size_of::<u16>();
const DATA_FREE_OFFSET: usize = 2;
const MAP_RECORD_BYTES: usize =
    INLINE_BITMAP_OFFSET + QualifiedInlineRecord::PAGE_CAPACITY / u8::BITS as usize;

pub(super) struct Plan {
    pub append: State<FreshIndexOwner>,
    pub definition: Vec<u8>,
    pub pages: Vec<CreatedPage>,
    pub indexed_rows: usize,
    pub distinct: u32,
}

pub(super) fn prepare(
    image: &NativeImage,
    table: &str,
    request: &SecondaryIndexCreate<'_>,
    original: &Original,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Plan, EditError> {
    let population = Population::capture(
        image,
        &original.checked,
        original.column,
        original.descending,
        budget,
        cancelled,
    )?;
    original.layout.new_length(
        original
            .name
            .as_str()
            .len()
            .checked_mul(size_of::<u16>())
            .ok_or(InspectError::StructureLimit)?,
    )?;
    let owner = FreshIndexOwner::capture(
        image,
        table,
        request,
        population.body_pages(),
        budget,
        cancelled,
    )?;
    let descriptor = CreatedDescriptor {
        physical: owner.physical(),
        logical: owner.logical(),
        usage: owner.usage(),
        root: owner.root(),
    };
    let definition = layout::compose(
        image,
        original,
        descriptor,
        population.distinct(),
        budget,
        cancelled,
    )?;
    let mut pages = buffer::vector(population.body_pages() + 1, budget, cancelled)?;
    pages.push(CreatedPage {
        number: owner.container(),
        bytes: map_container(&owner, budget, cancelled)?,
    });
    pages.extend(population.build(owner.definition(), owner.root(), budget, cancelled)?);
    let mut append = State::with_owner(image, owner)?;
    append.preview(pages.len())?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    for (position, page) in pages.iter().enumerate() {
        reader.tick()?;
        append.claim_global(&global, page.number, &mut reader)?;
        if position == 0 {
            append.reserve_created_index_map(page.number, reader.budget, cancelled)?;
        } else {
            append.reserve_created_index_body(page.number, reader.budget, cancelled)?;
        }
    }
    Ok(Plan {
        append,
        definition,
        pages,
        indexed_rows: population.rows(),
        distinct: population.distinct(),
    })
}

fn map_container(
    owner: &FreshIndexOwner,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, InspectError> {
    let mut page = buffer::vector(PAGE_BYTES, budget, cancelled)?;
    budget.charge(Resource::Steps, PAGE_BYTES as u64)?;
    page.resize(PAGE_BYTES, 0);
    page[0] = DATA_KIND;
    page[1] = DATA_VERSION;
    let row = PAGE_BYTES - MAP_RECORD_BYTES;
    page[DATA_FREE_OFFSET..DATA_FREE_OFFSET + size_of::<u16>()].copy_from_slice(
        &u16::try_from(row - DATA_SLOT_OFFSET - size_of::<u16>())
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    page[DATA_ROW_COUNT_OFFSET..DATA_ROW_COUNT_OFFSET + size_of::<u16>()]
        .copy_from_slice(&1_u16.to_le_bytes());
    page[DATA_SLOT_OFFSET..DATA_SLOT_OFFSET + size_of::<u16>()].copy_from_slice(
        &u16::try_from(row)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    page[row] = INLINE_MAP;
    page[row + 1..row + INLINE_BITMAP_OFFSET].copy_from_slice(&owner.root().to_le_bytes());
    for relative in 0..owner.bodies() {
        check(budget, cancelled)?;
        page[row + INLINE_BITMAP_OFFSET + relative / u8::BITS as usize] |=
            1 << (relative % u8::BITS as usize);
    }
    Ok(page)
}

pub(super) fn compose(
    image: &NativeImage,
    original: &Original,
    plan: &Plan,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    if plan.definition.len() != PAGE_BYTES
        || plan.append.original_length != image.as_bytes().len()
        || plan.append.length
            != image
                .as_bytes()
                .len()
                .checked_add(
                    plan.pages
                        .len()
                        .checked_mul(PAGE_BYTES)
                        .ok_or(InspectError::StructureLimit)?,
                )
                .ok_or(InspectError::StructureLimit)?
    {
        return Err(EditError::Preservation);
    }
    let mut after = buffer::vector(plan.append.length, budget, cancelled)?;
    for chunk in image
        .as_bytes()
        .chunks(crate::preservation::COPY_CHUNK_BYTES)
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        after.extend_from_slice(chunk);
    }
    budget
        .charge(Resource::Steps, (plan.append.length - after.len()) as u64)
        .map_err(InspectError::from)?;
    after.resize(plan.append.length, 0);
    let definition = original.layout.definition as usize * PAGE_BYTES;
    after[definition..definition + PAGE_BYTES].copy_from_slice(&plan.definition);
    for edit in &plan.append.map_edits {
        check(budget, cancelled)?;
        let byte = after.get_mut(edit.offset).ok_or(EditError::Preservation)?;
        if *byte != edit.before {
            return Err(EditError::Conflict);
        }
        *byte = edit.after;
    }
    for (position, page) in plan.pages.iter().enumerate() {
        check(budget, cancelled)?;
        let start = image.as_bytes().len() + position * PAGE_BYTES;
        if page.number as usize * PAGE_BYTES != start || page.bytes.len() != PAGE_BYTES {
            return Err(EditError::Preservation);
        }
        after[start..start + PAGE_BYTES].copy_from_slice(&page.bytes);
    }
    check(budget, cancelled)?;
    Ok(after)
}
