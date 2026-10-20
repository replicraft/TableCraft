//! Bounded native scalar and stored-collection row display.
use super::error::Error;
use crate::{native, output::TableOutput};
use tablecraft_access::{NativeBrowser, TablePageRequest};
use tablecraft_core::values::Value;

mod all;
mod collections;
mod multivalue;

pub(crate) const DEFAULT_PAGE_ROWS: u32 = 100;
pub(crate) const MAX_PAGE_ROWS: u32 = TablePageRequest::MAX_ROWS;

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Explicit MDB/ACCDB file path.
    pub(super) path: std::path::PathBuf,
    /// Exact table name (quote names containing spaces in your shell).
    #[arg(required_unless_present = "all", conflicts_with = "all")]
    pub(super) table: Option<String>,
    /// Display every user table in catalog order, with limit/offset per table.
    #[arg(long)]
    pub(super) all: bool,
    /// Maximum displayed rows; native read/resource ceilings still apply.
    #[arg(long, default_value_t = DEFAULT_PAGE_ROWS, value_parser = clap::value_parser!(u32).range(1..=MAX_PAGE_ROWS as i64))]
    pub(super) limit: u32,
    /// Skip this many live roots; collection key/child validation still runs.
    #[arg(long, default_value_t = 0)]
    pub(super) offset: u32,
    /// Include qualified stored calculations, marked as cached; may be stale, never recalculated.
    #[arg(long)]
    pub(super) stored_calculations: bool,
    /// Include qualified multivalue cells as stored lists; does not run lookup queries.
    #[arg(long)]
    pub(super) multivalues: bool,
    /// Include qualified multivalues, attachment metadata and stored Memo history.
    #[arg(long, conflicts_with = "multivalues")]
    pub(super) collections: bool,
}

impl Args {
    pub(crate) fn run(
        self,
        format: crate::presentation::Format,
        password: Option<&str>,
    ) -> Result<String, Error> {
        let mut budget = native::budget();
        let source = native::open(&self.path, password, &mut budget).map_err(Error::Read)?;
        let request = TablePageRequest::new(self.offset, self.limit).map_err(Error::Read)?;
        let mut browser = NativeBrowser::new(source, native::CATALOG_SCOPE, budget, &|| false);
        if self.all {
            all::run(&mut browser, &self, request, format)
        } else {
            let table = self
                .table
                .as_deref()
                .ok_or(Error::Read(tablecraft_ace::InspectError::Malformed))?;
            let text = read_page(&mut browser, table, request, &self)?;
            format.render(text).map_err(Error::Output)
        }
    }
}

fn read_page<R: tablecraft_ace::NativeReadSource>(
    browser: &mut NativeBrowser<'_, R>,
    table: &str,
    request: TablePageRequest,
    args: &Args,
) -> Result<String, Error> {
    let Args {
        collections,
        multivalues,
        stored_calculations,
        ..
    } = args;
    if *collections {
        let page = browser
            .collection_page(table, request, *stored_calculations)
            .map_err(Error::Read)?;
        return collections::render(&page);
    }
    if *multivalues {
        let page = browser
            .multivalue_page(table, request, *stored_calculations)
            .map_err(Error::Read)?;
        return multivalue::render(&page);
    }
    if *stored_calculations {
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
