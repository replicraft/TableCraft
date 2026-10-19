//! Read-only table discovery and paged display, without SQL construction/evaluation.
use crate::{inspect::Error, native, output::TableOutput};
use std::{fs::File, path::Path};
use tablecraft_ace::{inspect_catalog, read_table};
use tablecraft_model::ObjectKind;

pub(crate) const DEFAULT_PAGE_ROWS: u32 = 100;
pub(crate) const MAX_PAGE_ROWS: u32 = 16_384;

pub(crate) fn tables(path: &Path) -> Result<String, Error> {
    let mut source = File::open(path).map_err(|e| Error::Read(e.into()))?;
    let catalog = inspect_catalog(
        &mut source,
        native::CATALOG_SCOPE,
        &mut native::budget(),
        &|| false,
    )
    .map_err(Error::Read)?;
    let mut output = TableOutput::new();
    output.header(Some("Name"), 1).map_err(Error::Output)?;
    output.newline().map_err(Error::Output)?;
    // Inventory only: linked, hidden and system tables are listed, not opened.
    // No unqualified flag interpretation or name-prefix filtering hides entries.
    for object in catalog.catalog().objects() {
        if object.kind() == ObjectKind::Table {
            output
                .header(Some(object.name().as_str()), 1)
                .map_err(Error::Output)?;
            output.newline().map_err(Error::Output)?;
        }
    }
    Ok(output.finish())
}

pub(crate) fn rows(path: &Path, table: &str, limit: u32, offset: u32) -> Result<String, Error> {
    let mut source = File::open(path).map_err(|e| Error::Read(e.into()))?;
    let snapshot = read_table(
        &mut source,
        native::CATALOG_SCOPE,
        table,
        &mut native::budget(),
        &|| false,
    )
    .map_err(Error::Read)?;
    let mut output = TableOutput::new();
    for (index, column) in snapshot.columns().iter().enumerate() {
        if index != 0 {
            output.separator().map_err(Error::Output)?;
        }
        output
            .header(Some(column.name()), index + 1)
            .map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)?;
    // This is output pagination over a completed bounded snapshot, not lazy I/O.
    for row in snapshot
        .rows()
        .iter()
        .skip(offset as usize)
        .take(limit as usize)
    {
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
