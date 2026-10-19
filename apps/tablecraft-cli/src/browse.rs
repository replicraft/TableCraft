//! Read-only table discovery and paged display, without SQL construction/evaluation.
use crate::{inspect::Error, native, output::TableOutput};
use std::{fs::File, path::Path};
use tablecraft_access::{NativeBrowser, NativeTableStorage, TablePageRequest};

pub(crate) const DEFAULT_PAGE_ROWS: u32 = 100;
pub(crate) const MAX_PAGE_ROWS: u32 = TablePageRequest::MAX_ROWS;

pub(crate) fn tables(path: &Path, details: bool) -> Result<String, Error> {
    let source = File::open(path).map_err(|e| Error::Read(e.into()))?;
    let mut browser =
        NativeBrowser::new(source, native::CATALOG_SCOPE, native::budget(), &|| false);
    let inventory = browser.tables().map_err(Error::Read)?;
    let mut output = TableOutput::new();
    output.header(Some("Name"), 1).map_err(Error::Output)?;
    if details {
        output.separator().map_err(Error::Output)?;
        output.header(Some("Storage"), 2).map_err(Error::Output)?;
        output.separator().map_err(Error::Output)?;
        output.header(Some("Rows"), 3).map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)?;
    for entry in inventory.entries() {
        let object = entry.object();
        output
            .header(Some(object.name().as_str()), 1)
            .map_err(Error::Output)?;
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
            output.header(Some(storage), 2).map_err(Error::Output)?;
            output.separator().map_err(Error::Output)?;
            output.header(Some(rows), 3).map_err(Error::Output)?;
        }
        output.newline().map_err(Error::Output)?;
    }
    Ok(output.finish())
}

pub(crate) fn rows(path: &Path, table: &str, limit: u32, offset: u32) -> Result<String, Error> {
    let source = File::open(path).map_err(|e| Error::Read(e.into()))?;
    let request = TablePageRequest::new(offset, limit).map_err(Error::Read)?;
    let mut browser =
        NativeBrowser::new(source, native::CATALOG_SCOPE, native::budget(), &|| false);
    let page = browser.page(table, request).map_err(Error::Read)?;
    let mut output = TableOutput::new();
    for (index, column) in page.columns().iter().enumerate() {
        if index != 0 {
            output.separator().map_err(Error::Output)?;
        }
        output
            .header(Some(column.name()), index + 1)
            .map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)?;
    for row in page.rows() {
        for (index, value) in row.iter().enumerate() {
            if index != 0 {
                output.separator().map_err(Error::Output)?;
            }
            output.value(value).map_err(Error::Output)?;
        }
        output.newline().map_err(Error::Output)?;
    }
    Ok(output.finish())
}
