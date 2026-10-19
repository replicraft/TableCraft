//! Read-only native table inventory.
use super::error::Error;
use crate::{native, output::TableOutput};
use std::{fs::File, path::Path};
use tablecraft_access::{NativeBrowser, NativeTableStorage};

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Explicit MDB/ACCDB file path; no linked sources or active content are opened.
    pub(super) path: std::path::PathBuf,
    /// Show declared local/linked storage without opening tables or external sources.
    #[arg(long)]
    pub(super) details: bool,
    /// Include tables marked as system objects in the native catalog.
    #[arg(long)]
    pub(super) show_system: bool,
    /// Show stored local row counts (metadata, not a record-integrity audit).
    #[arg(long)]
    pub(super) count: bool,
}

pub(crate) fn run(
    path: &Path,
    details: bool,
    show_system: bool,
    count: bool,
) -> Result<String, Error> {
    let source = File::open(path).map_err(|e| Error::Read(e.into()))?;
    let mut browser =
        NativeBrowser::new(source, native::CATALOG_SCOPE, native::budget(), &|| false);
    let inventory = browser.tables().map_err(Error::Read)?;
    let mut output = TableOutput::new();
    output.header(Some("Name"), 1).map_err(Error::Output)?;
    output.separator().map_err(Error::Output)?;
    output.header(Some("Kind"), 2).map_err(Error::Output)?;
    if count {
        output.separator().map_err(Error::Output)?;
        output.header(Some("Row Count"), 3).map_err(Error::Output)?;
    }
    if details {
        output.separator().map_err(Error::Output)?;
        output.header(Some("Storage"), 3).map_err(Error::Output)?;
        output.separator().map_err(Error::Output)?;
        output.header(Some("Rows"), 4).map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)?;
    for entry in inventory
        .entries()
        .filter(|e| show_system || !e.is_system())
    {
        let object = entry.object();
        output
            .header(Some(object.name().as_str()), 1)
            .map_err(Error::Output)?;
        output.separator().map_err(Error::Output)?;
        output
            .header(Some(if entry.is_system() { "System" } else { "User" }), 2)
            .map_err(Error::Output)?;
        if count {
            output.separator().map_err(Error::Output)?;
            if entry.storage() == Some(NativeTableStorage::Local) {
                let description = browser
                    .describe(object.name().as_str())
                    .map_err(Error::Read)?;
                output
                    .display(&description.schema().declared_rows())
                    .map_err(Error::Output)?;
            } else {
                output.display(&"N/A").map_err(Error::Output)?;
            }
        }
        if details {
            let (storage, rows) = match entry.storage() {
                Some(NativeTableStorage::Local) => ("Local", "Not inspected"),
                Some(NativeTableStorage::Linked) => {
                    ("Linked non-ODBC", "External source not opened")
                }
                Some(NativeTableStorage::LinkedOdbc) => {
                    ("Linked ODBC", "External source not opened")
                }
                None => ("Unknown", "Not inspected"),
            };
            output.separator().map_err(Error::Output)?;
            output.header(Some(storage), 3).map_err(Error::Output)?;
            output.separator().map_err(Error::Output)?;
            output.header(Some(rows), 4).map_err(Error::Output)?;
        }
        output.newline().map_err(Error::Output)?;
    }
    Ok(output.finish())
}
