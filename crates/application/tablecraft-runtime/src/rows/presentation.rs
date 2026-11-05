//! Presentation lineage for known-origin native table rows.
//! Arbitrary SQL results cannot opt into native row identity through this helper.
use super::{controls, formatting};
use crate::{Cell, Column, Error, controls::SCOPE, display::Presentation};
use tablecraft_ace::{
    InspectError, NativeReadSource, TableProperties, TableSchema, inspect_table_schema,
};
use tablecraft_core::limits::ResourceBudget;

pub(crate) struct Metadata {
    schema: TableSchema,
    properties: Option<TableProperties>,
}

impl Metadata {
    pub fn inspect<R: NativeReadSource>(
        source: &mut R,
        table: &str,
        work: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let properties = crate::native_properties::inspect(source, table, work, cancelled)?;
        let schema = inspect_table_schema(source, SCOPE, table, work, cancelled)?;
        crate::native_properties::require_binding(properties.as_ref(), &schema)?;
        Ok(Self { schema, properties })
    }

    pub fn column(
        &self,
        position: usize,
        name: &str,
        output: &mut Presentation<'_>,
    ) -> Result<Column, Error> {
        output.work(1)?;
        let native = self
            .schema
            .columns()
            .get(position)
            .ok_or(InspectError::Malformed)?;
        output.work(native.name().len() as u64 + name.len() as u64)?;
        if native.name() != name {
            return Err(InspectError::Malformed.into());
        }

        crate::column::native(name, self.properties.as_ref(), output)
    }

    pub fn enrich(
        &self,
        columns: &[Column],
        rows: &mut [Vec<Cell>],
        output: &mut Presentation<'_>,
    ) -> Result<(), Error> {
        output.work(1)?;
        if columns.len() != self.schema.columns().len() {
            return Err(InspectError::Malformed.into());
        }

        controls::enrich(self.properties.as_ref(), columns, rows, output)?;
        formatting::apply(self.properties.as_ref(), columns, rows, output)
    }
}
