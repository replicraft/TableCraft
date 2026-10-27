//! Original borrowed requests define row authority without converting Int32 input.
use super::super::{EditError, field, physical, rows};
use super::Operation;
use crate::{TableSchema, format::reader::Reader};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(super) trait Scope {
    fn table(&self) -> &str;
    fn column(&self) -> &str;
    fn request_count(&self) -> usize;
    fn unique(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError>;
}

pub(super) trait Binding<const WIDTH: usize>: Scope {
    fn kind(&self, schema: &TableSchema) -> Result<ScalarType, EditError>;
    fn patches(
        &self,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
        schema: &TableSchema,
        position: usize,
    ) -> Result<Vec<physical::Patch<WIDTH>>, EditError>;
}

impl Scope for Operation<'_> {
    fn table(&self) -> &str {
        self.table
    }

    fn column(&self) -> &str {
        self.column
    }

    fn request_count(&self) -> usize {
        self.requests.len()
    }

    fn unique(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        rows::unique(self.requests, budget, cancelled)
    }
}

impl Binding<{ size_of::<i32>() }> for Operation<'_> {
    fn kind(&self, _schema: &TableSchema) -> Result<ScalarType, EditError> {
        Ok(ScalarType::Int32)
    }

    fn patches(
        &self,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
        schema: &TableSchema,
        position: usize,
    ) -> Result<Vec<physical::Patch<{ size_of::<i32>() }>>, EditError> {
        physical::patches(reader, schema, position, self.requests)
    }
}

pub(super) fn position<const WIDTH: usize>(
    operation: &impl Binding<WIDTH>,
    schema: &TableSchema,
) -> Result<(usize, ScalarType), EditError> {
    let kind = operation.kind(schema)?;
    let position = field::fixed_column(schema, operation.column(), kind, WIDTH)?;
    Ok((position, kind))
}
