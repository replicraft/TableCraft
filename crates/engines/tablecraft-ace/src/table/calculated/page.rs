//! Explicitly cached calculated results mixed with ordinary stored scalar cells.
use super::super::{TableFeatures, TablePageRequest, inspect_table_features, page, snapshot};
use super::read::qualified_kind;
use crate::InspectError;
use std::io::{Read, Seek};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::{Column, NameComparison, validate_columns};

/// A bounded page that explicitly permits qualified cached calculations.
/// No expression is evaluated and stored results may be stale or inconsistent
/// with ordinary columns. This is not an ordinary query snapshot or edit handle.
pub struct StoredTablePage {
    features: TableFeatures,
    columns: Vec<Column>,
    records: page::PageRecords,
    request: TablePageRequest,
}
impl StoredTablePage {
    /// Original inert native schema and expression/result-type declarations.
    pub fn features(&self) -> &TableFeatures {
        &self.features
    }
    /// Qualified carriers, in native logical order; cached columns remain nullable.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }
    /// Whether a column contains cached results. None means out of range.
    pub fn is_stored_calculation(&self, column: usize) -> Option<bool> {
        self.features.columns().nth(column).map(|c| c.calculated())
    }
    /// Exact stored values in physical order, with no freshness guarantee.
    pub fn rows(&self) -> &[Vec<Value>] {
        &self.records.rows
    }
    /// Original checked physical range.
    pub const fn request(&self) -> TablePageRequest {
        self.request
    }
    /// Whether enumeration reached EOF and checked the declared live-root count.
    /// As for scalar pages, a full page can stop before the actual end.
    pub const fn reached_end(&self) -> bool {
        self.records.reached_end
    }
}

/// Read a bounded page with ordinary scalars and explicitly cached calculations.
/// All declarations are qualified before reading rows, including empty pages.
/// Only the existing ACE 16-family stored Boolean/Int32/Double/Currency/DateTime/
/// Short Text wrappers with matching ResultType and nonempty Expression qualify.
/// Complex/unknown columns fail; no expression, provider or active content runs.
/// Skipped/later payloads are unchecked. Use a stable/offline source. Budgets,
/// cancellation and atomic failure follow the ordinary scalar paging contract.
pub fn read_stored_table_page<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    request: TablePageRequest,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<StoredTablePage, InspectError> {
    let features = inspect_table_features(source, scope, table, budget, cancelled)?;
    let count = features.schema().columns().len();
    budget.charge_allocation(count as u64, size_of::<Column>() as u64)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for field in features.columns() {
        crate::header::check(budget, cancelled)?;
        let column = field.column();
        let carrier = if field.calculated() {
            snapshot::projected_column(column, qualified_kind(&field)?, budget)?
        } else {
            budget.charge_allocation(column.name().len() as u64, 1)?;
            column.column()?
        };
        columns.push(carrier);
    }
    validate_columns(&columns, NameComparison::AsciiInsensitive)?;
    let records = page::read_records(
        source,
        features.schema(),
        &columns,
        request,
        budget,
        cancelled,
    )?;
    Ok(StoredTablePage {
        features,
        columns,
        records,
        request,
    })
}
