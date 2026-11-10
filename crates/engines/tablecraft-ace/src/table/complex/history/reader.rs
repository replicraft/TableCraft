//! Source-bound history membership, with stored values read for one parent.
mod result;

pub use result::ParentVersionHistory;

use super::{read::RECORD_FIELDS, record, schema};
use crate::{
    ComplexParentLink, ComplexTableSchema, InspectError, NativeColumn, NativeReadSource,
    ReadProfile, TableSnapshotLimits,
    header::check,
    table::complex::{
        inspect_complex_schema,
        links::directory::{Directory, Projection, payload::ParentRead, reference_positions},
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

/// Complete source-bound history keys with explicitly selected stored payloads.
/// The input names the visible Memo; its current value is never replaced by a
/// stored version. Every parent request repeats complete metadata/key validation
/// and child logical ROW membership through EOF. Only that parent's history rows
/// are decoded. Key-containing DATA pages still contain inline payload bytes.
/// This requires a stable/offline source and supplies no native lock or live-file
/// coherence. Source, budget and cancellation stay borrowed until reader drop.
pub struct VersionHistoryReader<'a, R> {
    source: &'a mut R,
    scope: CatalogScope,
    directory: Directory,
    payload: Projection,
    memo_position: usize,
    history_position: usize,
    limits: TableSnapshotLimits,
    budget: &'a mut ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
    failed: bool,
}

impl<'a, R: NativeReadSource> VersionHistoryReader<'a, R> {
    /// Resolve actual visible-Memo metadata and its qualified hidden history key.
    /// Opening checks complete membership without decoding current Memo or stored
    /// history Text/DATE payloads. All actual flat scalar metadata still qualifies.
    pub fn open(
        source: &'a mut R,
        scope: CatalogScope,
        table: &str,
        memo: &str,
        limits: TableSnapshotLimits,
        budget: &'a mut ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let _span = tracing::debug_span!("native.complex.history_reader.open").entered();
        let definition = inspect_complex_schema(source, scope, table, budget, cancelled)?;
        if !matches!(
            definition.owner().header.read_profile()?,
            ReadProfile::Ace2007 | ReadProfile::Ace2016 | ReadProfile::Ace2019
        ) {
            return Err(InspectError::Unsupported);
        }
        let (field_position, memo_position) = schema::select(&definition, memo, budget, cancelled)?;
        let field = definition
            .owner()
            .columns()
            .get(field_position)
            .ok_or(InspectError::Malformed)?;
        let mut selected = None;
        for column in definition.columns() {
            check(budget, cancelled)?;
            if column.number() == field.number() {
                selected = Some(column);
            }
        }
        let declaration = selected.ok_or(InspectError::Malformed)?;
        let flat = declaration.flat_table();
        schema::qualify(declaration, flat, budget, cancelled)?;
        let references = reference_positions(declaration, flat, budget, cancelled)?;
        let keys = Projection::capture(declaration, &references, budget, cancelled)?;
        // Preserve full-reader declaration coverage, including additional flat
        // columns. They must not become eligible merely because a view omits them.
        let columns = super::super::references::columns(declaration, flat, budget, cancelled)?;
        budget.charge_allocation(columns.len() as u64, size_of::<usize>() as u64)?;
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(columns.len())
            .map_err(|_| InspectError::Allocation)?;
        for position in 0..columns.len() {
            check(budget, cancelled)?;
            positions.push(position);
        }
        let payload = Projection::from_columns(positions, columns, &references, budget, cancelled)?;
        let directory = Directory::capture(
            source,
            definition,
            field_position,
            keys,
            limits,
            budget,
            cancelled,
        )?;
        check(budget, cancelled)?;
        Ok(Self {
            source,
            scope,
            directory,
            payload,
            memo_position,
            history_position: field_position,
            limits,
            budget,
            cancelled,
            failed: false,
        })
    }

    /// Complete captured owner, conceptual template and hidden flat definitions.
    pub fn schema(&self) -> &ComplexTableSchema {
        &self.directory.schema
    }

    /// Actual visible Memo declaration, not a synthesized latest stored value.
    pub fn memo_column(&self) -> &NativeColumn {
        &self.directory.schema.owner().columns()[self.memo_position]
    }

    /// Complete unfiltered parent keys and child membership in logical root order.
    pub fn parents(&self) -> &[ComplexParentLink] {
        self.directory.parents()
    }

    /// Whether an operational failure made the reader terminal.
    pub const fn is_failed(&self) -> bool {
        self.failed
    }

    /// Read one complete selected parent's stored history in its original order.
    /// Invalid ordinals leave a ready reader unchanged. Other failures publish no
    /// partial result and make later reads return CollectionReaderFailed.
    pub fn read_parent(&mut self, parent: usize) -> Result<ParentVersionHistory, InspectError> {
        let _span = tracing::debug_span!("native.complex.history_reader.parent").entered();
        if self.failed {
            return Err(InspectError::CollectionReaderFailed);
        }
        if parent >= self.directory.parents().len() {
            return Err(InspectError::InvalidPageRange);
        }
        let result = self.read_payload(parent);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn read_payload(&mut self, parent: usize) -> Result<ParentVersionHistory, InspectError> {
        let link = self
            .directory
            .parents()
            .get(parent)
            .ok_or(InspectError::InvalidPageRange)?;
        let reference = link.row_reference();
        let key = link.key();
        let declaration = self.directory.column();
        let columns = schema::qualify(
            declaration,
            declaration.flat_table(),
            self.budget,
            self.cancelled,
        )?;
        let cancelled = self.cancelled;
        let memo = self.directory.schema.owner().columns()[self.memo_position].name();
        let expected_positions = (self.history_position, self.memo_position);
        let rows = self.directory.read_parent(
            ParentRead {
                source: &mut *self.source,
                scope: self.scope,
                limits: self.limits,
                budget: &mut *self.budget,
                cancelled,
            },
            parent,
            &self.payload,
            |observed, count, budget| {
                // Unrelated complex templates participate in visible-Memo
                // uniqueness too. Reuse the actual fresh selector before any
                // result envelope allocation, after full parent refresh.
                if schema::select(observed, memo, budget, cancelled)? != expected_positions {
                    return Err(InspectError::Malformed);
                }
                budget.charge_allocation(count as u64, size_of::<Vec<Value>>() as u64)?;
                let mut rows = Vec::new();
                rows.try_reserve_exact(count)
                    .map_err(|_| InspectError::Allocation)?;
                Ok(rows)
            },
            |rows, row, budget| {
                check(budget, cancelled)?;
                budget.charge(Resource::Steps, RECORD_FIELDS)?;
                record::read(&row, &columns)?;
                rows.push(row);
                Ok(())
            },
        )?;
        Ok(ParentVersionHistory {
            reference,
            key,
            rows,
            columns,
        })
    }
}
