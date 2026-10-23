//! Sequential page reads without constructing a complete table snapshot.
use super::{TableSchema, record, snapshot};
use crate::{
    InspectError,
    format::{
        allocation,
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        reader::{Reader, row_count, row_range},
        row::{DELETED_MASK, slot_offset},
    },
    limits::MAX_LIVE_ROWS,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::{Column, validate_row};

mod cursor;
mod records;
mod traversal;
pub use cursor::{
    TableBatch, TableCursor, TableSnapshotLimits, read_table_batched, read_table_projection_batched,
};
pub(super) use records::read_records_batched;

/// A checked range of live rows in native physical traversal order.
/// It is not a persistent cursor or an ordering guarantee across changed files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TablePageRequest {
    offset: u32,
    limit: u32,
}
impl TablePageRequest {
    /// Maximum decoded/returned rows per page; work budgets may be stricter.
    pub const MAX_ROWS: u32 = MAX_LIVE_ROWS as u32;
    /// Select a live-row offset and nonzero page size, without offset arithmetic.
    pub fn new(offset: u32, limit: u32) -> Result<Self, InspectError> {
        if limit == 0 || limit > Self::MAX_ROWS {
            return Err(InspectError::InvalidPageRange);
        }
        Ok(Self { offset, limit })
    }
    /// Number of live roots skipped before decoding selected records.
    pub const fn offset(self) -> u32 {
        self.offset
    }
    /// Maximum decoded/returned records.
    pub const fn limit(self) -> u32 {
        self.limit
    }
}

/// An owned bounded page of scalar records, not a complete integrity audit.
/// All column layouts and selected values are qualified. Skipped/unvisited row
/// payloads, relocation targets and long values are not decoded or validated.
pub struct TablePage {
    schema: TableSchema,
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    request: TablePageRequest,
    reached_end: bool,
}
impl TablePage {
    /// Native schema, including the unverified declared total row count.
    pub fn schema(&self) -> &TableSchema {
        &self.schema
    }
    /// Validated scalar columns in native logical order.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }
    /// Selected records in physical traversal order.
    pub fn rows(&self) -> &[Vec<Value>] {
        &self.rows
    }
    /// The checked requested range.
    pub const fn request(&self) -> TablePageRequest {
        self.request
    }
    /// Whether all allocated data-page root slots were enumerated. Only then
    /// was the declared live-row count checked. A full page may stop before EOF;
    /// another request can be needed even when it was the last full page.
    pub const fn reached_end(&self) -> bool {
        self.reached_end
    }
}

/// Read a bounded page from an explicitly stable/offline local scalar table.
///
/// All schema columns must be supported. Enumeration skips deleted roots, counts
/// live roots without decoding skipped payloads, and stops when the page is full.
/// Only selected live rows consume the row allowance, in addition to metadata
/// reads. Every visited slot/page consumes steps; metadata/page reads and selected
/// allocations share the caller's byte budget. Larger total tables are permitted
/// within the existing file, physical-read and caller limits. Usage metadata is
/// fully validated before data reads; addresses are then enumerated without the
/// complete snapshot's bounded address list. No query filtering, provider
/// invocation or shared-file snapshot acquisition occurs.
/// Failure returns no partial page; use full reads/checks for whole-table audits.
pub fn read_table_page<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    name: &str,
    request: TablePageRequest,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TablePage, InspectError> {
    let schema = super::inspect_table_schema(source, scope, name, budget, cancelled)?;
    let columns = snapshot::columns(&schema, budget)?;
    let PageRecords { rows, reached_end } =
        read_records(source, &schema, &columns, request, budget, cancelled)?;
    Ok(TablePage {
        schema,
        columns,
        rows,
        request,
        reached_end,
    })
}

pub(super) struct PageRecords {
    pub rows: Vec<Vec<Value>>,
    pub reached_end: bool,
}

// Shared physical traversal; callers must qualify every supplied column first.
pub(super) fn read_records<R: crate::NativeReadSource>(
    source: &mut R,
    schema: &TableSchema,
    columns: &[Column],
    request: TablePageRequest,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PageRecords, InspectError> {
    let mut reader = Reader {
        source,
        header: schema.header,
        budget,
        cancelled,
        reads: 0,
    };
    let mut traversal = traversal::Traversal::new(&mut reader, schema)?;
    let mut rows = Vec::new();
    while rows.len() < request.limit as usize {
        let Some(root) = traversal.next(&mut reader, schema)? else {
            break;
        };
        if traversal.visited() <= u64::from(request.offset) {
            traversal.skip(&root)?;
            continue;
        }
        let values = traversal.values(&root, &mut reader, schema, columns, None)?;
        push(&mut rows, values, &mut reader)?;
    }
    reader.tick()?;
    Ok(PageRecords {
        rows,
        reached_end: traversal.ended(),
    })
}

fn push<R: crate::NativeReadSource>(
    rows: &mut Vec<Vec<Value>>,
    values: Vec<Value>,
    reader: &mut Reader<'_, R>,
) -> Result<(), InspectError> {
    reader
        .budget
        .charge_allocation(1, size_of::<Vec<Value>>() as u64)?;
    rows.try_reserve(1).map_err(|_| InspectError::Allocation)?;
    rows.push(values);
    Ok(())
}
