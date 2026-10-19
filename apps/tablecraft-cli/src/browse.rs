//! Read-only table discovery and paged display, without SQL construction/evaluation.
use crate::{inspect::Error, native, output::TableOutput};
use std::{fs::File, path::Path};
use tablecraft_access::{NativeBrowser, NativeTableStorage, TablePageRequest};
use tablecraft_core::values::Value;

mod multivalue;

pub(crate) const DEFAULT_PAGE_ROWS: u32 = 100;
pub(crate) const MAX_PAGE_ROWS: u32 = TablePageRequest::MAX_ROWS;

pub(crate) fn tables(path: &Path, details: bool, hide_system: bool) -> Result<String, Error> {
    let source = File::open(path).map_err(|e| Error::Read(e.into()))?;
    let mut browser =
        NativeBrowser::new(source, native::CATALOG_SCOPE, native::budget(), &|| false);
    let inventory = browser.tables().map_err(Error::Read)?;
    let mut output = TableOutput::new();
    output.header(Some("Name"), 1).map_err(Error::Output)?;
    output.separator().map_err(Error::Output)?;
    output.header(Some("Kind"), 2).map_err(Error::Output)?;
    if details {
        output.separator().map_err(Error::Output)?;
        output.header(Some("Storage"), 3).map_err(Error::Output)?;
        output.separator().map_err(Error::Output)?;
        output.header(Some("Rows"), 4).map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)?;
    for entry in inventory
        .entries()
        .filter(|e| !hide_system || !e.is_system())
    {
        let object = entry.object();
        output
            .header(Some(object.name().as_str()), 1)
            .map_err(Error::Output)?;
        output.separator().map_err(Error::Output)?;
        output
            .header(Some(if entry.is_system() { "System" } else { "User" }), 2)
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
            output.header(Some(storage), 3).map_err(Error::Output)?;
            output.separator().map_err(Error::Output)?;
            output.header(Some(rows), 4).map_err(Error::Output)?;
        }
        output.newline().map_err(Error::Output)?;
    }
    Ok(output.finish())
}

pub(crate) fn rows(
    path: &Path,
    table: &str,
    limit: u32,
    offset: u32,
    stored_calculations: bool,
    multivalues: bool,
) -> Result<String, Error> {
    let source = File::open(path).map_err(|e| Error::Read(e.into()))?;
    let request = TablePageRequest::new(offset, limit).map_err(Error::Read)?;
    let mut browser =
        NativeBrowser::new(source, native::CATALOG_SCOPE, native::budget(), &|| false);
    if multivalues {
        let page = browser
            .multivalue_page(table, request, stored_calculations)
            .map_err(Error::Read)?;
        return multivalue::render(&page);
    }
    if stored_calculations {
        let page = browser.stored_page(table, request).map_err(Error::Read)?;
        display_rows(
            page.columns()
                .iter()
                .enumerate()
                .map(|(i, c)| (c.name(), page.is_stored_calculation(i) == Some(true))),
            page.rows(),
        )
    } else {
        let page = browser.page(table, request).map_err(Error::Read)?;
        display_rows(
            page.columns().iter().map(|c| (c.name(), false)),
            page.rows(),
        )
    }
}

fn display_rows<'a>(
    columns: impl Iterator<Item = (&'a str, bool)>,
    rows: &[Vec<Value>],
) -> Result<String, Error> {
    let mut output = TableOutput::new();
    for (index, (name, cached)) in columns.enumerate() {
        if index != 0 {
            output.separator().map_err(Error::Output)?;
        }
        output
            .header(Some(name), index + 1)
            .map_err(Error::Output)?;
        if cached {
            output.display(&" [cached]").map_err(Error::Output)?;
        }
    }
    output.newline().map_err(Error::Output)?;
    for row in rows {
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
