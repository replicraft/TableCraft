//! Structured child metadata is separate from scalar/list values.
use crate::QueryError;
use std::sync::Arc;
use tablecraft_db::{MultivalueList, OwnedRecordset, ProjectedRow, RecordsetSchema};
use tablecraft_schema::{SchemaError, SchemaName};

/// One source-owned structured child recordset declaration. It supplies no
/// mutable identity, filesystem action, FileData value or implicit decoder.
pub struct RecordsetColumn {
    name: SchemaName,
    schema: Arc<RecordsetSchema>,
}

impl RecordsetColumn {
    /// Consume an actual field name and validated immutable child schema.
    /// Producers pay acquisition/ownership costs before publishing metadata.
    ///
    /// # Errors
    /// Returns the shared name validation error when the field name is empty
    /// or contains a control character. Source-specific name policies and duplicate
    /// declarations are checked separately during query binding.
    pub fn new(name: String, schema: Arc<RecordsetSchema>) -> Result<Self, SchemaError> {
        Ok(Self {
            name: SchemaName::new(name)?,
            schema,
        })
    }

    pub(crate) fn schema_name(&self) -> &SchemaName {
        &self.name
    }

    /// Borrow the exact source field name used to bind this recordset projection.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Borrow the shared schema; taking an independent handle never copies rows.
    pub fn schema(&self) -> &Arc<RecordsetSchema> {
        &self.schema
    }
}

/// One parent scalar projection, existing collection lists and complete owned
/// recordsets in their separately requested orders. Callback failures propagate.
pub type StructuredVisitor<'a> = dyn FnMut(
        ProjectedRow<'_>,
        &[&dyn MultivalueList],
        &[Arc<OwnedRecordset>],
    ) -> Result<(), QueryError>
    + 'a;

/// Sparse aligned output metadata. Scalar/list-only plans allocate no entries.
pub struct RecordsetOutput {
    pub(crate) index: usize,
    pub(crate) schema: Arc<RecordsetSchema>,
}

impl RecordsetOutput {
    /// Zero-based output position, independent of scalar and list sidecar indices.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Borrow the captured child declarations and inspection availability,
    /// including when execution returns no parent rows for this output.
    pub fn schema(&self) -> &RecordsetSchema {
        &self.schema
    }
}

/// Compare captured metadata structurally under the execution owner's paid,
/// cancellable work callback; sharing the same Arc is not source revalidation.
pub(crate) fn recordsets_match<'a>(
    expected: impl ExactSizeIterator<Item = &'a RecordsetColumn>,
    actual: &[RecordsetColumn],
    mut work: impl FnMut(usize) -> Result<(), QueryError>,
) -> Result<bool, QueryError> {
    work(1)?;
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (expected, actual) in expected.zip(actual) {
        work(
            expected
                .name()
                .len()
                .max(actual.name().len())
                .checked_add(1)
                .ok_or(QueryError::StructureLimit)?,
        )?;
        if expected.name != actual.name {
            return Ok(false);
        }
        if !schemas_match(expected.schema(), actual.schema(), &mut work)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn schemas_match(
    expected: &RecordsetSchema,
    actual: &RecordsetSchema,
    mut work: impl FnMut(usize) -> Result<(), QueryError>,
) -> Result<bool, QueryError> {
    work(1)?;
    if expected.kind() != actual.kind()
        || expected.names() != actual.names()
        || expected.columns().len() != actual.columns().len()
    {
        return Ok(false);
    }
    for ((expected, actual), (left, right)) in expected
        .columns()
        .iter()
        .zip(actual.columns())
        .zip(expected.availability().iter().zip(actual.availability()))
    {
        work(
            expected
                .name()
                .len()
                .max(actual.name().len())
                .checked_add(1)
                .ok_or(QueryError::StructureLimit)?,
        )?;
        if expected != actual || left != right {
            return Ok(false);
        }
    }
    Ok(true)
}
