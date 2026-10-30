//! One original record becomes one complete fitted record and joint key cache.
use super::{
    request::{self, FittedColumnUpdates},
    serialization::RecordPolicy,
    variables,
};
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::{
        page::PAGE_BYTES,
        reader::Reader,
        row::{COLUMN_COUNT_BYTES, MASK_BITS, reference_page},
    },
    table::{
        indexes::RowKeyUpdate,
        mutation::{page_update, patch::BytePatch, variable_split::metadata::Metadata},
        record, record_layout,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) struct Record {
    pub reference: u32,
    pub offset: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub original_values: Vec<Value>,
    pub final_values: Vec<Value>,
}

impl BytePatch for Record {
    fn offset(&self) -> usize {
        self.offset
    }
    fn before(&self) -> &[u8] {
        &self.before
    }
    fn after(&self) -> &[u8] {
        &self.after
    }
    fn requires_structural_check(&self) -> bool {
        true
    }
}

pub(super) struct Staged {
    pub affected: Vec<usize>,
    pub projection: Vec<usize>,
    pub records: Vec<Record>,
}

impl Staged {
    pub fn keys(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<RowKeyUpdate<'_>>, EditError> {
        budget
            .charge_allocation(
                self.records.len() as u64,
                size_of::<RowKeyUpdate<'_>>() as u64,
            )
            .map_err(InspectError::from)?;
        let mut keys = Vec::new();
        keys.try_reserve_exact(self.records.len())
            .map_err(|_| InspectError::Allocation)?;
        for row in &self.records {
            request::check(budget, cancelled)?;
            keys.push(RowKeyUpdate {
                reference: row.reference,
                columns: &self.projection,
                before: &row.original_values,
                after: &row.final_values,
            });
        }
        Ok(keys)
    }
}

pub(super) fn build(
    image: &NativeImage,
    checked: &TableCheck,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    build_with(
        image,
        checked,
        groups,
        RecordPolicy::FittedDeclared,
        budget,
        cancelled,
    )
}

pub(super) fn build_with(
    image: &NativeImage,
    checked: &TableCheck,
    groups: &[FittedColumnUpdates<'_>],
    policy: RecordPolicy,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    let schema = checked.schema();
    let bound = request::bind(schema, groups, budget, cancelled)?;
    crate::table::indexes::qualify_row_indexes(
        schema,
        checked.indexes(),
        &bound.columns,
        budget,
        cancelled,
    )?;

    budget
        .charge_allocation(schema.columns().len() as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    let mut projection = Vec::new();
    projection
        .try_reserve_exact(schema.columns().len())
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge(Resource::Steps, checked.indexes().len() as u64)
        .map_err(InspectError::from)?;
    let key_columns = checked.indexes().iter().try_fold(0_usize, |count, index| {
        count
            .checked_add(index.index().columns().len())
            .ok_or(InspectError::StructureLimit)
    })?;
    let work = key_columns
        .checked_mul(bound.columns.len() + 1)
        .and_then(|count| count.checked_add(bound.columns.len()))
        .ok_or(InspectError::StructureLimit)?;

    for (position, field) in schema.columns().iter().enumerate() {
        request::check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, work as u64)
            .map_err(InspectError::from)?;
        let sibling = checked.indexes().iter().any(|index| {
            index.index().columns().iter().any(|key| {
                bound
                    .columns
                    .iter()
                    .any(|&selected| schema.columns()[selected].number() == key.number())
            }) && index
                .index()
                .columns()
                .iter()
                .any(|key| key.number() == field.number())
        });
        if bound.columns.contains(&position) || sibling {
            projection.push(position);
        }
    }

    let captured = page_update::roots::capture(image, schema, &bound.rows, budget, cancelled)?;

    if matches!(policy, RecordPolicy::FittedDeclared) {
        let metadata = Metadata::capture_for_fitted_updates(
            image,
            schema,
            checked.indexes(),
            &bound.columns,
            budget,
            cancelled,
        )?;
        for root in &captured.selected {
            metadata.require_data_page(reference_page(root.reference), budget, cancelled)?;
            let terminal = u32::try_from(root.final_record.offset / PAGE_BYTES)
                .map_err(|_| InspectError::StructureLimit)?;
            metadata.require_data_page(terminal, budget, cancelled)?;
        }
    }
    budget
        .charge_allocation(captured.selected.len() as u64, size_of::<Record>() as u64)
        .map_err(InspectError::from)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(captured.selected.len())
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
        let before = root.final_record.data;
        let layout = record_layout::parse(&before, schema, reader.budget)?;
        if layout.count != schema.layout.max_columns {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let original_values =
            record::decode_selected(&before, schema, Some(&projection), &mut reader)?;
        let mut changed = false;
        for (group, &column) in groups.iter().zip(&bound.columns) {
            reader.work(group.len() + projection.len())?;
            if let Some(cell) = (0..group.len())
                .filter_map(|i| group.cell(i))
                .find(|cell| cell.row == root.update.0)
            {
                let at = projection
                    .iter()
                    .position(|&candidate| candidate == column)
                    .ok_or(EditError::Preservation)?;
                let value = original_values.get(at).ok_or(EditError::Preservation)?;
                if !request::values_equal(value, cell.expected, reader.budget, cancelled)? {
                    return Err(EditError::Conflict);
                }
                changed |=
                    !request::values_equal(cell.expected, cell.value, reader.budget, cancelled)?;
            }
        }
        if !changed {
            continue;
        }

        reader
            .budget
            .charge_allocation(
                bound.variable_groups.len() as u64,
                size_of::<variables::Cell<'_>>() as u64,
            )
            .map_err(InspectError::from)?;
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(bound.variable_groups.len())
            .map_err(|_| InspectError::Allocation)?;
        for &group_number in &bound.variable_groups {
            reader.tick()?;
            let group = &groups[group_number];
            reader.work(group.len())?;
            let cell = (0..group.len())
                .filter_map(|position| group.cell(position))
                .find(|cell| cell.row == root.update.0);
            if let Some(cell) = cell
                && !request::values_equal(cell.expected, cell.value, reader.budget, cancelled)?
            {
                cells.push(variables::Cell {
                    column: bound.columns[group_number],
                    value: cell.value,
                });
            }
        }
        let mut after = variables::rebuild(
            schema,
            &before,
            &layout,
            &cells,
            policy,
            reader.budget,
            cancelled,
        )?;
        if matches!(policy, RecordPolicy::FittedDeclared) && after.len() != before.len() {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }

        let final_layout = match policy {
            RecordPolicy::FittedDeclared => layout,
            RecordPolicy::GrowingPreferred => record_layout::parse(&after, schema, reader.budget)?,
        };
        for (group, &column) in groups.iter().zip(&bound.columns) {
            if !matches!(group, FittedColumnUpdates::Fixed { .. }) {
                continue;
            }
            reader.work(group.len())?;
            let Some(cell) = (0..group.len())
                .filter_map(|i| group.cell(i))
                .find(|cell| cell.row == root.update.0)
            else {
                continue;
            };
            let field = &schema.columns()[column];
            let (bytes, width, present) = request::fixed_bytes(field, cell.value)?;
            if present && width != 0 {
                let start = COLUMN_COUNT_BYTES + field.field.offset;
                let end = start
                    .checked_add(width)
                    .ok_or(InspectError::StructureLimit)?;
                if end > final_layout.data_end {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
                after
                    .get_mut(start..end)
                    .ok_or(InspectError::Malformed)?
                    .copy_from_slice(&bytes[..width]);
            }
            let at = final_layout.mask_start + field.number() / MASK_BITS;
            let bit = 1 << (field.number() % MASK_BITS);
            let mask = after.get_mut(at).ok_or(InspectError::Malformed)?;
            *mask = (*mask & !bit) | if present { bit } else { 0 };
        }

        let final_values = record::decode_selected(&after, schema, Some(&projection), &mut reader)?;
        records.push(Record {
            reference: root.reference,
            offset: root.final_record.offset,
            before,
            after,
            original_values,
            final_values,
        });
    }
    Ok(Staged {
        affected: bound.columns,
        projection,
        records,
    })
}

pub(super) fn copy(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    request::check(budget, cancelled)?;
    budget
        .charge_allocation(bytes.len() as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(bytes.len())
        .map_err(|_| InspectError::Allocation)?;
    result.extend_from_slice(bytes);
    Ok(result)
}
