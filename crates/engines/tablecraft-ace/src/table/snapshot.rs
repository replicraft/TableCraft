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
    let _span = tracing::debug_span!("native.table.read").entered();
    let schema = inspect_table_schema(source, scope, name, budget, cancelled)?;
    if schema.declared_rows() as usize > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit);
    }
    let columns = columns(&schema, budget)?;
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
            let values = record::decode(&row, &schema, &mut reader)?;
            validate_row(&columns, &values)?;
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
    Ok(TableSnapshot {
        schema,
        columns,
        rows,
    })
}

pub(super) fn columns(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
) -> Result<Vec<Column>, InspectError> {
    schema.scalar_columns_with_budget(budget)
}
