use super::{
    record,
    schema::{TableSchema, inspect_table_schema},
};
use crate::{
    InspectError,
    format::{
        allocation,
        bytes::{u16_at, u32_at},
        reader::{Reader, row_count},
    },
};
use crate::{
    format::{
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        row::{DELETED_MASK, slot_offset},
    },
    limits::MAX_LIVE_ROWS,
};
use std::io::{Read, Seek};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::{Column, validate_row};

/// Completed bounded scalar snapshot of one explicit local table.
/// No active content, property validation or provider is invoked.
/// The read entry point chooses sequential or qualified native index traversal.
pub struct TableSnapshot {
    pub(super) schema: TableSchema,
    pub(super) columns: Vec<Column>,
    pub(super) rows: Vec<Vec<Value>>,
}
impl TableSnapshot {
    /// Consume the native snapshot without cloning validated scalar data. The
    /// metadata remains native; adapters own any shared snapshot/identity policy.
    pub fn into_parts(self) -> (TableSchema, Vec<Column>, Vec<Vec<Value>>) {
        (self.schema, self.columns, self.rows)
    }
    pub(crate) fn into_rows(self) -> Vec<Vec<Value>> {
        self.rows
    }

    /// Native table/column metadata used to decode this snapshot.
    pub fn schema(&self) -> &TableSchema {
        &self.schema
    }
    /// Shared exact-carrier input columns.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }
    /// Live rows in the selected traversal order, without a SQL ordering guarantee.
    pub fn rows(&self) -> &[Vec<Value>] {
        &self.rows
    }
}
/// Decode all live rows of one local scalar table from a stable/offline source.
/// Every column must be supported, even when a future consumer projects fewer.
/// Failure returns no partial snapshot. Structural ceilings and caller budgets
/// bound work; no TOP/WHERE pushdown or native shared-file snapshot is implied.
pub fn read_table<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    read_selected(source, scope, name, None, budget, cancelled)
}

// Internal metadata consumers may project catalog fields without interpreting
// unrelated object bodies. Public table reads still require all scalar columns.
pub(crate) fn read_selected<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    name: &str,
    selected: Option<&[&str]>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    let _span = tracing::debug_span!("native.table.read").entered();
    let schema = inspect_table_schema(source, scope, name, budget, cancelled)?;
    if schema.declared_rows() as usize > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit);
    }
    let mut positions = Vec::new();
    let columns = if let Some(names) = selected {
        if names.is_empty() || names.len() > schema.columns.len() {
            return Err(InspectError::Malformed);
        }
        budget.charge_allocation(names.len() as u64, size_of::<usize>() as u64)?;
        positions
            .try_reserve_exact(names.len())
            .map_err(|_| InspectError::Allocation)?;
        let mut columns = Vec::new();
        budget.charge_allocation(names.len() as u64, size_of::<Column>() as u64)?;
        columns
            .try_reserve_exact(names.len())
            .map_err(|_| InspectError::Allocation)?;
        for (position, column) in schema.columns.iter().enumerate() {
            if names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(column.name()))
            {
                budget.charge_allocation(column.name().len() as u64, 1)?;
                columns.push(column.column()?);
                positions.push(position);
            }
        }
        if positions.len() != names.len() {
            return Err(InspectError::Malformed);
        }
        columns
    } else {
        columns(&schema, budget)?
    };
    let rows = read_records(
        source,
        &schema,
        &columns,
        selected.map(|_| positions.as_slice()),
        budget,
        cancelled,
    )?;
    Ok(TableSnapshot {
        schema,
        columns,
        rows,
    })
}

// Explicit feature readers may supply a separately qualified projected carrier.
// Ordinary scalar reads still qualify every requested column before this path.
pub(super) fn read_records<R: Read + Seek>(
    source: &mut R,
    schema: &TableSchema,
    columns: &[Column],
    selected: Option<&[usize]>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Value>>, InspectError> {
    if schema.declared_rows() as usize > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit);
    }
    let mut reader = Reader {
        source,
        header: schema.header,
        budget,
        cancelled,
        reads: 0,
    };
    let pages = allocation::pages(&mut reader, schema.layout.usage)?;
    let mut rows = Vec::new();
    for page_number in pages {
        let page = reader.page(page_number)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&page, OWNER_OFFSET)? != schema.definition
        {
            return Err(InspectError::Malformed);
        }
        for slot in 0..row_count(&page)? {
            reader.tick()?;
            if u16_at(&page, slot_offset(slot))? & DELETED_MASK != 0 {
                continue;
            }
            if rows.len() >= MAX_LIVE_ROWS {
                return Err(InspectError::StructureLimit);
            }
            reader.budget.charge(Resource::Rows, 1)?;
            let row = reader.row(page_number, slot as u8, Some(schema.definition))?;
            let values = record::decode_selected(&row, schema, selected, &mut reader)?;
            validate_row(columns, &values)?;
            reader
                .budget
                .charge_allocation(1, std::mem::size_of::<Vec<Value>>() as u64)?;
            rows.try_reserve(1).map_err(|_| InspectError::Allocation)?;
            rows.push(values);
        }
    }
    if rows.len() != schema.declared_rows() as usize {
        return Err(InspectError::Malformed);
    }
    reader.tick()?;
    tracing::debug!(
        rows = rows.len(),
        columns = columns.len(),
        "native table completed"
    );
    Ok(rows)
}

pub(super) fn columns(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
) -> Result<Vec<Column>, InspectError> {
    schema.scalar_columns_with_budget(budget)
}
