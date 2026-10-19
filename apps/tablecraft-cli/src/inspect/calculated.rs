use super::{Error, line};
use crate::{native, output::TableOutput};
use std::fs::File;

pub(super) fn run(
    source: &mut File,
    table: &str,
    column: &str,
    output: &mut TableOutput,
) -> Result<(), Error> {
    let stored = tablecraft_ace::read_stored_calculation(
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
    let heading = format!("Stored {:?} value", stored.data_type());
    output.table_section().map_err(Error::Output)?;
    line(output, &["Row ordinal", &heading])?;
    for (ordinal, value) in stored.values().iter().enumerate() {
        output
            .header(Some(&ordinal.to_string()), 1)
            .map_err(Error::Output)?;
        output.separator().map_err(Error::Output)?;
        output.value(value).map_err(Error::Output)?;
        output.newline().map_err(Error::Output)?;
    }
    Ok(())
}
