//! Actual value preconditions, new chunks and stable-root descriptor replacement.
use super::super::{
    EditError, SlotScalarUpdate, WriteBlocker,
    page_update::{record, roots, roots::Root, space::Space},
};
use super::{
    admission::Prepared,
    encode::{self, Encoded, StorageChoice},
    maps::{Family, Maps},
    ownership::Inventory,
    placement::Plan,
    value,
};
use crate::{
    InspectError, NativeImage,
    format::{
        page::PAGE_BYTES,
        reader::Reader,
        row::{reference_page, slot_offset},
    },
    header::check,
    table::record as native_record,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

const REFERENCE_BYTES: usize = size_of::<u32>();
const MAX_ORDINARY_BODY_BYTES: usize = PAGE_BYTES - slot_offset(1);

pub(super) struct Change<'u> {
    pub root: Root<'u>,
    pub encoded: Encoded,
    pub choice: StorageChoice,
    pub descriptor: Vec<u8>,
    pub chunks: Vec<usize>,
    pub target: Range<usize>,
}

pub(super) struct Staged<'u> {
    pub plan: Plan,
    pub changes: Vec<Change<'u>>,
}

pub(super) fn build<'u>(
    image: &NativeImage,
    prepared: &Prepared,
    updates: &'u [SlotScalarUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged<'u>, EditError> {
    let schema = prepared.structural.schema();
    let column = prepared.column;
    let field = &schema.columns()[column];
    let report = prepared
        .storage
        .columns()
        .iter()
        .find(|report| report.column_number() == field.number())
        .ok_or(InspectError::Malformed)?;
    let references = report.maps().ok_or(InspectError::Malformed)?;
    let maps = Maps::capture(image, schema, references, budget, cancelled)?;
    let inventory = Inventory::capture(image, budget, cancelled)?;
    inventory.require_exclusive(
        schema,
        field.number(),
        maps.pages(Family::LongValue),
        budget,
        cancelled,
    )?;
    let captured = roots::capture(image, schema, updates, budget, cancelled)?;
    let space = Space::capture(image, schema, &captured.owned, budget, cancelled)?;
    let mut plan = Plan::new(image, schema.definition_page(), maps);
    budget
        .charge_allocation(updates.len() as u64, size_of::<Change<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut changes = Vec::new();
    changes
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };

    for root in captured.selected {
        reader.tick()?;
        let values = native_record::decode_selected(
            &root.final_record.data,
            schema,
            Some(&[column]),
            &mut reader,
        )?;
        if values.len() != 1
            || !value::equal(
                &values[0],
                root.update.expected(),
                reader.budget,
                reader.cancelled,
            )?
        {
            return Err(EditError::Conflict);
        }
        if value::equal(
            root.update.expected(),
            root.update.value(),
            reader.budget,
            reader.cancelled,
        )? {
            space.require_page(
                image,
                reference_page(root.reference),
                false,
                reader.budget,
                reader.cancelled,
            )?;
            let final_page = u32::try_from(root.final_record.offset / PAGE_BYTES)
                .map_err(|_| InspectError::Malformed)?;
            space.require_page(image, final_page, false, reader.budget, reader.cancelled)?;
            continue;
        }

        let encoded = encode::encode(field, root.update.value(), reader.budget, reader.cancelled)?;
        let mut choice = encoded.preferred();
        if choice == StorageChoice::Inline {
            let descriptor = encoded.descriptor(choice, None, reader.budget, reader.cancelled)?;
            let inline = record::replace_payload(
                schema,
                column,
                &root.final_record.data,
                &descriptor,
                !matches!(encoded, Encoded::Null),
                reader.budget,
                reader.cancelled,
            )?;
            if inline.len() > MAX_ORDINARY_BODY_BYTES {
                choice = match &encoded {
                    Encoded::Present(bytes) if !bytes.is_empty() => StorageChoice::Single,
                    _ => return Err(EditError::Unsupported(WriteBlocker::RowLayout)),
                };
            }
        }
        let chunks = encoded.chunks(choice, reader.budget, reader.cancelled)?;
        reader
            .budget
            .charge_allocation(chunks.len() as u64, size_of::<usize>() as u64)
            .map_err(InspectError::from)?;
        let mut placed = Vec::new();
        placed
            .try_reserve_exact(chunks.len())
            .map_err(|_| InspectError::Allocation)?;
        for chunk in &chunks {
            reader.tick()?;
            let body = encoded.body(chunk, 0, reader.budget, reader.cancelled)?;
            placed.push(plan.append(
                image,
                Family::LongValue,
                &body,
                reader.budget,
                reader.cancelled,
            )?);
        }
        for (position, chunk) in chunks.iter().enumerate() {
            reader.tick()?;
            if !chunk.linked {
                continue;
            }
            let next = placed
                .get(position + 1)
                .map_or(0, |&index| plan.appended[index].reference);
            let range = plan.appended[placed[position]].range.clone();
            let link = range.start..range.start + REFERENCE_BYTES;
            plan.bytes_mut(link, reader.budget, reader.cancelled)?
                .copy_from_slice(&next.to_le_bytes());
        }
        let first = placed.first().map(|&index| plan.appended[index].reference);
        let descriptor = encoded.descriptor(choice, first, reader.budget, reader.cancelled)?;
        let record = record::replace_payload(
            schema,
            column,
            &root.final_record.data,
            &descriptor,
            !matches!(encoded, Encoded::Null),
            reader.budget,
            reader.cancelled,
        )?;
        let target = plan.ordinary(
            image,
            &root,
            &record,
            &space,
            reader.budget,
            reader.cancelled,
        )?;
        reader
            .budget
            .charge(Resource::Steps, record.len() as u64)
            .map_err(InspectError::from)?;
        if plan.bytes_mut(target.clone(), reader.budget, reader.cancelled)? != record {
            return Err(EditError::Preservation);
        }
        if target.len() < root.final_record.data.len() {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        changes.push(Change {
            root,
            encoded,
            choice,
            descriptor,
            chunks: placed,
            target,
        });
    }
    check(reader.budget, reader.cancelled)?;

    Ok(Staged { plan, changes })
}
