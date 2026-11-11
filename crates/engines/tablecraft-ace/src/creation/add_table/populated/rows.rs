//! Complete borrowed payload validation and dynamic row lengths before image allocation.
use super::packing;
use crate::{
    InspectError,
    creation::{
        CreateDatabaseError,
        input::{Cell, Columns, Rows},
        profile::Fields,
        record::{self, DataPage, UserRow},
        request::Request,
        variable::{self, Payload},
    },
    format::{page::PAGE_BYTES, row::SLOT_BYTES},
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

pub(in crate::creation::add_table) struct Prepared<'a> {
    pub rows: Rows<'a>,
    pub pages: Vec<packing::Page>,
    pub minimum_row: usize,
    lengths: Vec<usize>,
    payloads: Vec<Payload<'a>>,
    variables: usize,
}

pub(in crate::creation::add_table) enum Membership {
    Owned,
    Candidates,
}

fn length(
    columns: Columns<'_>,
    rows: Rows<'_>,
    row: usize,
    minimum: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, CreateDatabaseError> {
    check(budget, cancelled)?;
    if rows.width(row) != columns.count() {
        return Err(CreateDatabaseError::RowWidth);
    }
    let mut length = minimum;
    for column in 0..columns.count() {
        check(budget, cancelled)?;
        let Cell::Value(value) = rows.cell(row, column) else {
            return Err(CreateDatabaseError::Carrier);
        };
        let scalar = columns.scalar(column);
        if matches!(value, Value::Null) {
            if scalar == ScalarType::Boolean {
                return Err(CreateDatabaseError::Carrier);
            }
        } else if ScalarType::of(value) != Some(scalar) {
            return Err(CreateDatabaseError::Carrier);
        }
        if matches!(
            scalar,
            ScalarType::Text | ScalarType::Binary | ScalarType::Decimal
        ) {
            let payload = variable::capture(columns, rows, row, column, budget, cancelled)?;
            length = length
                .checked_add(payload.byte_len())
                .ok_or(CreateDatabaseError::Layout)?;
        }
    }
    if length
        .checked_add(SLOT_BYTES)
        .is_none_or(|bytes| bytes > PAGE_BYTES - record::DATA_HEADER_BYTES)
    {
        return Err(CreateDatabaseError::PageCapacity);
    }
    Ok(length)
}

impl<'a> Prepared<'a> {
    pub fn capture(
        columns: Columns<'a>,
        rows: Rows<'a>,
        fields: Fields<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreateDatabaseError> {
        u32::try_from(rows.count()).map_err(|_| CreateDatabaseError::Layout)?;
        let variables = fields.variable_count();
        let footer = if variables == 0 {
            0
        } else {
            (variables + 2) * size_of::<u16>()
        };
        let minimum_row = fields
            .fixed_end()
            .checked_add(footer)
            .and_then(|value| value.checked_add(columns.count().div_ceil(u8::BITS as usize)))
            .ok_or(CreateDatabaseError::Layout)?;
        // Complete immutable-input preflight precedes every retained row-plan or
        // replacement-image allocation; no native row count is inferred from fit.
        for row in 0..rows.count() {
            length(columns, rows, row, minimum_row, budget, cancelled)?;
        }
        let payload_count = rows
            .count()
            .checked_mul(variables)
            .ok_or(CreateDatabaseError::Layout)?;
        budget
            .charge_allocation(rows.count() as u64, size_of::<usize>() as u64)
            .map_err(InspectError::from)?;
        budget
            .charge_allocation(payload_count as u64, size_of::<Payload<'_>>() as u64)
            .map_err(InspectError::from)?;
        let mut lengths = Vec::new();
        lengths
            .try_reserve_exact(rows.count())
            .map_err(|_| InspectError::Allocation)?;
        let mut payloads = Vec::new();
        payloads
            .try_reserve_exact(payload_count)
            .map_err(|_| InspectError::Allocation)?;
        for row in 0..rows.count() {
            lengths.push(length(columns, rows, row, minimum_row, budget, cancelled)?);
            for column in 0..columns.count() {
                if matches!(
                    columns.scalar(column),
                    ScalarType::Text | ScalarType::Binary | ScalarType::Decimal
                ) {
                    payloads.push(variable::capture(
                        columns, rows, row, column, budget, cancelled,
                    )?);
                }
            }
        }
        let pages = packing::capture(&lengths, budget, cancelled)?;
        Ok(Self {
            rows,
            pages,
            minimum_row,
            lengths,
            payloads,
            variables,
        })
    }

    pub fn write(
        &self,
        after: &mut [u8],
        data_start: usize,
        definition: usize,
        request: &Request<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CreateDatabaseError> {
        let columns = request.columns;
        let fields = request.fields();
        for (page, packed) in self.pages.iter().enumerate() {
            check(budget, cancelled)?;
            let number = data_start
                .checked_add(page)
                .ok_or(CreateDatabaseError::Layout)?;
            let start = number
                .checked_mul(PAGE_BYTES)
                .ok_or(CreateDatabaseError::Layout)?;
            let destination = after
                .get_mut(start..start + PAGE_BYTES)
                .ok_or(CreateDatabaseError::Layout)?;
            let mut writer = DataPage::new(destination, definition)?;
            for index in packed.rows.start..packed.rows.end {
                let payload_start = index * self.variables;
                let payloads = (self.variables != 0)
                    .then_some(&self.payloads[payload_start..payload_start + self.variables]);
                let record = writer.append(self.lengths[index], budget, cancelled)?;
                record::user_row(
                    record,
                    UserRow {
                        fields,
                        columns,
                        cells: self.rows,
                        index,
                        payloads,
                        length: self.lengths[index],
                    },
                    budget,
                    cancelled,
                )?;
            }
            writer.finish()?;
        }
        Ok(())
    }

    pub fn members(
        &self,
        start: usize,
        membership: Membership,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u32>, InspectError> {
        budget.charge_allocation(self.pages.len() as u64, size_of::<u32>() as u64)?;
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(self.pages.len())
            .map_err(|_| InspectError::Allocation)?;
        for (offset, page) in self.pages.iter().enumerate() {
            check(budget, cancelled)?;
            if matches!(membership, Membership::Owned) || page.candidate(self.minimum_row) {
                pages.push(
                    u32::try_from(
                        start
                            .checked_add(offset)
                            .ok_or(InspectError::StructureLimit)?,
                    )
                    .map_err(|_| InspectError::StructureLimit)?,
                );
            }
        }
        Ok(pages)
    }
}
