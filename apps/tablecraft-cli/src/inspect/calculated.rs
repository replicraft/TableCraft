use super::{Error, line};
use crate::{native, output::TableOutput};
use std::fs::File;

pub(super) fn run(
    source: &mut File,
    table: &str,
    column: &str,
    output: &mut TableOutput,
) -> Result<(), Error> {
    let stored = tablecraft_ace::read_stored_int32_calculation(
        source,
        native::CATALOG_SCOPE,
        table,
        column,
        &mut native::budget(),
        &|| false,
    )
    .map_err(Error::Read)?;
    line(output, &["Table", stored.features().schema().name()])?;
    line(output, &["Column", stored.column().name()])?;
    line(output, &["Values", "Stored; not recalculated"])?;
    line(output, &["Row ordinal", "Stored Int32 value"])?;
    for (ordinal, value) in stored.values().iter().enumerate() {
        let value = value.map_or_else(|| "NULL".to_owned(), |value| value.to_string());
        line(output, &[&ordinal.to_string(), &value])?;
    }
    Ok(())
}
