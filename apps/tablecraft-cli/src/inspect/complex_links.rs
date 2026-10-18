use super::{Error, line};
use crate::{native, output::TableOutput};
use std::fs::File;

pub(super) fn run(
    source: &mut File,
    table: &str,
    column: &str,
    output: &mut TableOutput,
) -> Result<(), Error> {
    let links = tablecraft_ace::read_complex_column_links(
        source,
        native::CATALOG_SCOPE,
        table,
        column,
        &mut native::budget(),
        &|| false,
    )
    .map_err(Error::Read)?;
    line(output, &["Table", links.schema().owner().name()])?;
    let field = links
        .schema()
        .owner()
        .columns()
        .iter()
        .find(|field| field.number() == links.column().number())
        .ok_or(Error::Read(tablecraft_ace::InspectError::Malformed))?;
    line(output, &["Column", field.name()])?;
    line(
        output,
        &["Child table", links.child_snapshot().schema().name()],
    )?;
    line(output, &["Parent ordinal", "Complex key", "Raw child rows"])?;
    for (ordinal, parent) in links.parents().iter().enumerate() {
        let key = parent
            .key()
            .map_or_else(|| "NULL".to_owned(), |key| key.to_string());
        line(
            output,
            &[
                &ordinal.to_string(),
                &key,
                &parent.child_count().to_string(),
            ],
        )?;
    }
    Ok(())
}
