use super::{Error, collections, line};
use crate::{native, output::TableOutput};
use std::fs::File;
use tablecraft_access::NativeBrowser;

pub(super) fn run(
    source: &mut File,
    table: &str,
    memo: &str,
    output: &mut TableOutput,
) -> Result<(), Error> {
    let mut browser =
        NativeBrowser::new(source, native::CATALOG_SCOPE, native::budget(), &|| false);
    let stored = browser
        .version_history_column(table, memo)
        .map_err(Error::Read)?;
    collections::summary(
        output,
        stored.links(),
        stored.memo_column().name(),
        "Stored versions",
    )?;
    line(
        output,
        &[
            "Parent ordinal",
            "Identity",
            "Value",
            "Modified",
            "Modified bits",
        ],
    )?;
    for parent in 0..stored.links().parents().len() {
        for version in stored
            .stored_versions(parent)
            .ok_or(Error::Read(tablecraft_ace::InspectError::Malformed))?
        {
            let version = version.map_err(Error::Read)?;
            output.display(&parent).map_err(Error::Output)?;
            output.separator().map_err(Error::Output)?;
            output.display(&version.identity()).map_err(Error::Output)?;
            collections::text(output, version.value())?;
            collections::timestamp(output, version.modified())?;
            output.newline().map_err(Error::Output)?;
        }
    }
    Ok(())
}
