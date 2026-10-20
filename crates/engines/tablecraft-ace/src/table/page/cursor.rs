//! Source-bound resumable scalar paging, without serializable physical bookmarks.
use super::{traversal::Traversal, *};

/// An owned batch returned by a source-bound table cursor.
/// Schema/columns stay on the cursor rather than being cloned for each batch.
pub struct TableBatch {
    rows: Vec<Vec<Value>>,
    offset: u32,
    reached_end: bool,
}
impl TableBatch {
    /// Decoded rows in native physical order; this is not SQL ordering.
    pub fn rows(&self) -> &[Vec<Value>] {
        &self.rows
    }
    /// Number of rows successfully returned before this batch.
    pub const fn offset(&self) -> u32 {
        self.offset
    }
    /// Whether all live roots were enumerated and the declared count matched.
    /// A full batch may require another fetch to discover EOF.
    pub const fn reached_end(&self) -> bool {
        self.reached_end
    }
}

/// Resumable scalar reads tied to one exclusively borrowed stable/offline source.
///
/// Opening qualifies the requested scalar columns and all usage metadata. Fetches retain
/// the current data page/slot and map position, avoiding earlier-root rescans.
/// Each fetch has explicit caller controls and a fresh physical-read quota;
/// reusing a budget keeps cumulative accounting, supplying a new one is explicit.
/// This is not a snapshot, live-file lock or transferable/persisted bookmark.
///
/// Any fetch failure after range validation makes the cursor terminal: no partial
/// batch is published, no budget is refunded, and later fetches return CursorFailed.
/// Invalid ranges leave a ready cursor untouched. No retry is performed.
pub struct TableCursor<'a, R> {
    source: &'a mut R,
    schema: TableSchema,
    columns: Vec<Column>,
    selected: Option<Vec<usize>>,
    traversal: Traversal,
    delivered: u32,
    failed: bool,
}
impl<'a, R: Read + Seek> TableCursor<'a, R> {
    /// Qualify a named local scalar table without reading its user records.
    /// The source must remain offline/stable until the cursor is dropped.
    pub fn open(
        source: &'a mut R,
        scope: CatalogScope,
        name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        Self::open_selected(source, scope, name, None, budget, cancelled)
    }
    /// Qualify an explicit ordinary scalar selection, in native logical order.
    /// Names use the existing ASCII-insensitive native projection policy; missing
    /// or duplicate selections fail. Unselected payloads are not decoded or
    /// qualified, but complete row framing is still checked for each fetched row.
    /// An empty selection yields one zero-width row per fetched live root.
    /// Unsupported selected fields fail even on empty tables, without NULL substitutes.
    pub fn open_projection(
        source: &'a mut R,
        scope: CatalogScope,
        name: &str,
        selected: &[&str],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        Self::open_selected(source, scope, name, Some(selected), budget, cancelled)
    }
    fn open_selected(
        source: &'a mut R,
        scope: CatalogScope,
        name: &str,
        names: Option<&[&str]>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let schema = super::super::inspect_table_schema(source, scope, name, budget, cancelled)?;
        let (selected, columns) = if let Some(names) = names {
            let (positions, columns) =
                snapshot::projected_columns(&schema, names, budget, cancelled)?;
            (Some(positions), columns)
        } else {
            (None, snapshot::columns(&schema, budget)?)
        };
        let mut reader = Reader {
            source,
            header: schema.header,
            budget,
            cancelled,
            reads: 0,
        };
        let traversal = Traversal::new(&mut reader, &schema)?;
        reader.tick()?;
        Ok(Self {
            source,
            schema,
            columns,
            selected,
            traversal,
            delivered: 0,
            failed: false,
        })
    }
    /// Captured native schema, including the initially unverified row declaration.
    pub fn schema(&self) -> &TableSchema {
        &self.schema
    }
    /// Qualified shared scalar columns, reused by every batch.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }
    /// Number of rows in batches already returned successfully.
    pub const fn position(&self) -> u32 {
        self.delivered
    }
    /// Whether a failed fetch has permanently closed continuation.
    pub const fn is_failed(&self) -> bool {
        self.failed
    }
    /// Fetch a nonzero batch up to TablePageRequest::MAX_ROWS, without rescanning.
    /// Cancellation/resource failures publish no partial batch and close the cursor.
    pub fn next_page(
        &mut self,
        limit: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<TableBatch, InspectError> {
        if self.failed {
            return Err(InspectError::CursorFailed);
        }
        TablePageRequest::new(0, limit)?;
        let result = self.fetch(limit, budget, cancelled);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn fetch(
        &mut self,
        limit: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<TableBatch, InspectError> {
        let mut reader = Reader {
            source: &mut *self.source,
            header: self.schema.header,
            budget,
            cancelled,
            reads: 0,
        };
        let mut rows = Vec::new();
        while rows.len() < limit as usize {
            let Some(root) = self.traversal.next(&mut reader, &self.schema)? else {
                break;
            };
            let values = self.traversal.values(
                &root,
                &mut reader,
                &self.schema,
                &self.columns,
                self.selected.as_deref(),
            )?;
            push(&mut rows, values, &mut reader)?;
        }
        reader.tick()?;
        let offset = self.delivered;
        self.delivered = self
            .delivered
            .checked_add(
                rows.len()
                    .try_into()
                    .map_err(|_| InspectError::StructureLimit)?,
            )
            .ok_or(InspectError::StructureLimit)?;
        Ok(TableBatch {
            rows,
            offset,
            reached_end: self.traversal.ended(),
        })
    }
}
