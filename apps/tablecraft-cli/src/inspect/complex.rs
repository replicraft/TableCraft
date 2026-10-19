use super::{Error, line};
use crate::{native, output::TableOutput};
use std::io::{Read, Seek};
use tablecraft_ace::{InspectError, inspect_complex_schema};

pub(super) fn run<R: Read + Seek>(
    source: &mut R,
    name: &str,
    output: &mut TableOutput,
) -> Result<(), Error> {
    let schema = inspect_complex_schema(
        source,
        native::CATALOG_SCOPE,
        name,
        &mut native::budget(),
        &|| false,
    )
    .map_err(Error::Read)?;
    line(output, &["Table", schema.owner().name()])?;
    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &[
            "Column",
            "Complex type ID",
            "Type table",
            "Flat table",
            "Foreign key",
            "Child identity",
            "Declared children",
        ],
    )?;
    for column in schema.columns() {
        let find = |table: &tablecraft_ace::TableSchema, number| {
            table
                .columns()
                .iter()
                .position(|c| c.number() == number)
                .ok_or(Error::Read(InspectError::Malformed))
        };
        let owner_name = schema.owner().columns()[find(schema.owner(), column.number())?].name();
        let foreign_key = column.flat_table().columns()
            [find(column.flat_table(), column.foreign_key_number())?]
        .name();
        let identity = column.flat_table().columns()
            [find(column.flat_table(), column.identity_number())?]
        .name();
        line(
            output,
            &[
                owner_name,
                &column.type_id().to_string(),
                column.type_table().name(),
                column.flat_table().name(),
                foreign_key,
                identity,
                &column.flat_table().declared_rows().to_string(),
            ],
        )?;
    }
    Ok(())
}
