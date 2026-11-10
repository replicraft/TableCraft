//! Source-bound complete membership with explicitly selected parent Value decoding.

use super::{read::require_parent_counts, schema::qualified};
use crate::{
    ComplexParentLink, InspectError, NativeReadSource, NativeRowReference, NativeSortOrder,
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
use tablecraft_schema::Column;

/// One complete selected parent's stored multivalue list, not a column snapshot.
/// Duplicates, NULL Value cells and stored order are retained without coercion.
/// It cannot be used as the complete source/query collection membership cache.
pub struct ParentMultivalue {
    reference: NativeRowReference,
    key: Option<i32>,
    values: Vec<Value>,
}

impl ParentMultivalue {
    /// Original parent root, meaningful only with the issuing reader/source schema.
    pub const fn row_reference(&self) -> NativeRowReference {
        self.reference
    }
    /// Raw complex key in the selected column; NULL remains distinct from an assigned empty list.
    pub const fn key(&self) -> Option<i32> {
        self.key
    }
    /// Complete unfiltered membership of this parent in stored order.
    pub fn values(&self) -> &[Value] {
        &self.values
    }
}

/// Exclusively borrowed stable/offline source with a complete checked key directory.
/// Opening scans all keys through EOF but does not decode Value payloads. Each
/// read_parent rechecks the complete key/ROW census and decodes only that parent's
/// values. This bounds payload retention; it does not promise constant memory,
/// sublinear I/O, native locking, live-file coherence or a complete payload snapshot.
/// The original budget/callback remain cumulative until drop. Operational errors
/// after valid range checks are terminal; no implicit retry or partial cache exists.
pub struct MultivalueReader<'a, R> {
    source: &'a mut R,
    scope: CatalogScope,
    directory: Directory,
    values: Projection,
    value_column: Column,
    value_order: Option<NativeSortOrder>,
    limits: TableSnapshotLimits,
    budget: &'a mut ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
    failed: bool,
}

impl<'a, R: NativeReadSource> MultivalueReader<'a, R> {
    /// Resolve one actual stored multivalue declaration and complete key membership.
    /// Existing scalar/template/reference rules and every parent's 100-value bound
    /// apply before return, including empty or later-unrequested parents.
    pub fn open(
        source: &'a mut R,
        scope: CatalogScope,
        table: &str,
        column: &str,
        limits: TableSnapshotLimits,
        budget: &'a mut ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let _span = tracing::debug_span!("native.complex.multivalue_reader.open").entered();
        let schema = inspect_complex_schema(source, scope, table, budget, cancelled)?;
        let mut field_position = None;
        for (position, field) in schema.owner().columns().iter().enumerate() {
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
        let field = &schema.owner().columns()[field_position];
        if !super::super::supported_key(field) {
            return Err(InspectError::Unsupported);
        }
        let mut declaration_position = None;
        for (position, candidate) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            if candidate.number() == field.number() {
                declaration_position = Some(position);
            }
        }
        let declaration = schema
            .columns()
            .get(declaration_position.ok_or(InspectError::Unsupported)?)
            .ok_or(InspectError::Malformed)?;
        let flat = declaration.flat_table();
        let (value_position, _) = qualified(declaration, flat)?;
        // Preserve all actual complete-reader metadata/carrier checks; only the
        // unselected record Value payload is outside this explicit lazy view.
        let mut columns = super::super::references::columns(declaration, flat, budget, cancelled)?;
        if value_position >= columns.len() {
            return Err(InspectError::Malformed);
        }
        let value_order = flat.columns()[value_position].text_sort_order();
        let value_column = columns.swap_remove(value_position);
        let references = reference_positions(declaration, flat, budget, cancelled)?;
        let values = Projection::capture(
            declaration,
            &[references[0], references[1], value_position],
            budget,
            cancelled,
        )?;
        let keys = Projection::capture(declaration, &references, budget, cancelled)?;
        let directory = Directory::capture(
            source,
            schema,
            field_position,
            keys,
            limits,
            budget,
            cancelled,
        )?;
        require_parent_counts(directory.parents(), budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(Self {
            source,
            scope,
            directory,
            values,
            value_column,
            value_order,
            limits,
            budget,
            cancelled,
            failed: false,
        })
    }

    /// Captured actual owner/type/flat definitions for this source-bound directory.
    /// This metadata is not a source/revision certificate for another handle.
    pub fn schema(&self) -> &crate::ComplexTableSchema {
        &self.directory.schema
    }

    /// Complete validated parent key/membership entries in original live-root order.
    pub fn parents(&self) -> &[ComplexParentLink] {
        self.directory.parents()
    }
    /// Exact stored Value carrier; no lookup display translation is applied.
    pub fn value_column(&self) -> &Column {
        &self.value_column
    }
    /// Original flat Value Text descriptor, without template/database fallback.
    pub const fn value_text_sort_order(&self) -> Option<NativeSortOrder> {
        self.value_order
    }
    /// Whether an operational failure made this source-bound reader terminal.
    pub const fn is_failed(&self) -> bool {
        self.failed
    }

    /// Read one local parent ordinal after fresh complete key/ROW revalidation.
    /// Out-of-range input leaves a ready reader unchanged. An operational error
    /// returns no partial parent list and later reads return CollectionReaderFailed.
    pub fn read_parent(&mut self, parent: usize) -> Result<ParentMultivalue, InspectError> {
        let _span = tracing::debug_span!("native.complex.multivalue_reader.parent").entered();
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

    fn read_payload(&mut self, parent: usize) -> Result<ParentMultivalue, InspectError> {
        let link = self
            .directory
            .parents()
            .get(parent)
            .ok_or(InspectError::InvalidPageRange)?;
        let reference = link.row_reference();
        let key = link.key();
        let value_position = self.values.roles[2];
        let values = self.directory.read_parent(
            ParentRead {
                source: &mut *self.source,
                scope: self.scope,
                limits: self.limits,
                budget: &mut *self.budget,
                cancelled: self.cancelled,
            },
            parent,
            &self.values,
            |_observed, count, budget| {
                budget.charge_allocation(count as u64, size_of::<Value>() as u64)?;
                let mut values = Vec::new();
                values
                    .try_reserve_exact(count)
                    .map_err(|_| InspectError::Allocation)?;
                Ok(values)
            },
            |values, row, _budget| {
                let value = row
                    .into_iter()
                    .nth(value_position)
                    .ok_or(InspectError::Malformed)?;
                values.push(value);
                Ok(())
            },
        )?;
        Ok(ParentMultivalue {
            reference,
            key,
            values,
        })
    }
}
