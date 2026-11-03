//! Complete attachment membership with explicitly selected encoded payload reads.
mod result;

pub use result::ParentAttachments;

use super::{record, schema};
use crate::{
    ComplexParentLink, ComplexTableSchema, InspectError, NativeReadSource, ReadProfile,
    TableSnapshotLimits,
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

/// A stable/offline source with complete attachment key and logical ROW membership.
/// Opening reads key pages without decoding attachment fields or fetching
/// external FileData. Every selected-parent read repeats the
/// complete parent/child key census through EOF and decodes only that parent's
/// stored metadata and encoded FileData. No complete query cache is populated.
/// The source, budget and callback stay exclusively borrowed until drop. This
/// does not provide native locking, live-file coherence or constant-time access.
pub struct AttachmentReader<'a, R> {
    source: &'a mut R,
    scope: CatalogScope,
    directory: Directory,
    payload: Projection,
    limits: TableSnapshotLimits,
    budget: &'a mut ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
    failed: bool,
}

impl<'a, R: NativeReadSource> AttachmentReader<'a, R> {
    /// Qualify the actual attachment definition and complete key membership.
    /// Modern conceptual flags are accepted only by the attachment qualifier;
    /// they never become permission for generic scalar table reads.
    pub fn open(
        source: &'a mut R,
        scope: CatalogScope,
        table: &str,
        column: &str,
        limits: TableSnapshotLimits,
        budget: &'a mut ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let _span = tracing::debug_span!("native.complex.attachment_reader.open").entered();
        let definition = inspect_complex_schema(source, scope, table, budget, cancelled)?;
        if !matches!(
            definition.owner().header.read_profile()?,
            ReadProfile::Ace2007 | ReadProfile::Ace2016 | ReadProfile::Ace2019
        ) {
            return Err(InspectError::Unsupported);
        }
        let mut field_position = None;
        for (position, field) in definition.owner().columns().iter().enumerate() {
            check(budget, cancelled)?;
            let work = field
                .name()
                .len()
                .checked_add(column.len())
                .ok_or(InspectError::StructureLimit)?;
            budget.charge(Resource::Steps, work as u64)?;
            if field.name().eq_ignore_ascii_case(column) {
                field_position = Some(position);
                break;
            }
        }
        let field_position = field_position.ok_or(InspectError::UnknownColumn)?;
        let field = &definition.owner().columns()[field_position];
        if !super::super::supported_key(field) {
            return Err(InspectError::Unsupported);
        }
        let mut selected = None;
        for candidate in definition.columns() {
            check(budget, cancelled)?;
            if candidate.number() == field.number() {
                selected = Some(candidate);
            }
        }
        let declaration = selected.ok_or(InspectError::Unsupported)?;
        let flat = declaration.flat_table();
        let references = reference_positions(declaration, flat, budget, cancelled)?;
        let names = [
            flat.columns()[references[0]].name(),
            flat.columns()[references[1]].name(),
        ];
        // Pay the actual bounded name comparisons of the attachment-owned
        // key projection before its ASCII matching, including modern flags.
        for field in flat.columns() {
            for name in names {
                check(budget, cancelled)?;
                let work = field
                    .name()
                    .len()
                    .checked_add(name.len())
                    .ok_or(InspectError::StructureLimit)?;
                budget.charge(Resource::Steps, work as u64)?;
            }
        }
        let (positions, columns) =
            schema::projection(declaration, flat, Some(&names), budget, cancelled)?;
        let keys = Projection::from_columns(positions, columns, &references, budget, cancelled)?;
        let (positions, columns) = schema::projection(declaration, flat, None, budget, cancelled)?;
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
            limits,
            budget,
            cancelled,
            failed: false,
        })
    }

    /// Actual captured owner, conceptual template and flat child definitions.
    pub fn schema(&self) -> &ComplexTableSchema {
        &self.directory.schema
    }

    /// Complete unfiltered key membership in original live parent-root order.
    /// A child with NULL FileData still contributes to its parent's child count.
    pub fn parents(&self) -> &[ComplexParentLink] {
        self.directory.parents()
    }

    /// Whether an operational failure made this source-bound reader terminal.
    pub const fn is_failed(&self) -> bool {
        self.failed
    }

    /// Return a complete selected-parent list with owned encoded storage values.
    /// Out-of-range input leaves a ready reader unchanged. Other failures return
    /// no partial result and subsequent reads return CollectionReaderFailed.
    pub fn read_parent(&mut self, parent: usize) -> Result<ParentAttachments, InspectError> {
        let _span = tracing::debug_span!("native.complex.attachment_reader.parent").entered();
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

    fn read_payload(&mut self, parent: usize) -> Result<ParentAttachments, InspectError> {
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
            |_observed, count, budget| {
                budget.charge_allocation(count as u64, size_of::<Vec<Value>>() as u64)?;
                let mut rows = Vec::new();
                rows.try_reserve_exact(count)
                    .map_err(|_| InspectError::Allocation)?;
                Ok(rows)
            },
            |rows, row, budget| {
                check(budget, cancelled)?;
                budget.charge(Resource::Steps, schema::FIELD_COUNT as u64)?;
                record::read(&row, &columns)?;
                rows.push(row);
                Ok(())
            },
        )?;
        Ok(ParentAttachments {
            reference,
            key,
            rows,
            columns,
        })
    }
}
