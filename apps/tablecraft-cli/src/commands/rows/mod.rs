//! Bounded native scalar and stored-collection row display.
use super::error::Error;
use crate::{native, output::TableOutput};
use tablecraft_access::{BrowsePage, CollectionPageRequest, NativeBrowser, TablePageRequest};
use tablecraft_ace::{InspectError, TableSnapshotLimits};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_core::values::Value;

mod all;
mod collections;
mod multivalue;
pub(in crate::commands) mod stream;

pub(crate) const DEFAULT_PAGE_ROWS: u32 = 100;
pub(crate) const MAX_PAGE_ROWS: u32 = TablePageRequest::MAX_ROWS;

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Explicit Access database/package/template path.
    pub(super) path: std::path::PathBuf,
    /// Exact table name; omitted only when there is exactly one user table.
    #[arg(conflicts_with = "all")]
    pub(super) table: Option<String>,
    /// Display every user table in catalog order, with limit/offset per table.
    #[arg(long)]
    pub(super) all: bool,
    /// Stream all ordinary scalar rows as TSV batches; later errors may follow earlier output.
    #[arg(long, conflicts_with_all = ["all", "offset", "stored_calculations", "multivalues", "collections"])]
    pub(super) stream: bool,
    /// Ordinary scalar column to stream; repeat for a subset of a mixed schema.
    #[arg(long = "column", requires = "stream")]
    pub(super) columns: Vec<String>,
    /// Cumulative native read/container bytes, including temporary extraction and failures.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=native::MAX_QUERY_READ_BYTES))]
    pub(super) native_bytes: Option<u64>,
    /// Cumulative native rows; collection pages also use this as the parent/child table ceiling.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=native::MAX_NATIVE_READ_ROWS))]
    pub(super) native_rows: Option<u64>,
    /// Cumulative native work for buffered browsing or streaming.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=native::MAX_NATIVE_READ_STEPS))]
    pub(super) native_steps: Option<u64>,
    /// Maximum displayed rows, or rows per batch with --stream; read/resource ceilings still apply.
    #[arg(long, default_value_t = DEFAULT_PAGE_ROWS, value_parser = clap::value_parser!(u32).range(1..=MAX_PAGE_ROWS as i64))]
    pub(super) limit: u32,
    /// Skip this many live roots; collection key/child validation still runs.
    #[arg(long, default_value_t = 0)]
    pub(super) offset: u32,
    /// Compatibility flag: buffered browsing already includes marked stored calculation caches.
    #[arg(long)]
    pub(super) stored_calculations: bool,
    /// Include qualified multivalue cells as stored lists; does not run lookup queries.
    #[arg(long)]
    pub(super) multivalues: bool,
    /// Compatibility flag: buffered browsing already includes qualified stored collections.
    #[arg(long, conflicts_with = "multivalues")]
    pub(super) collections: bool,
}

impl Args {
    pub(crate) fn run(
        self,
        format: crate::presentation::Format,
        password: Option<&str>,
    ) -> Result<String, Error> {
        let page = TablePageRequest::new(self.offset, self.limit).map_err(Error::Read)?;
        let row_allowance = self.native_rows.unwrap_or(native::DEFAULT_NATIVE_ROWS);
        let maximum_rows =
            u32::try_from(row_allowance).map_err(|_| Error::Read(InspectError::StructureLimit))?;
        let limits =
            TableSnapshotLimits::new(maximum_rows, TableSnapshotLimits::default().batch_rows())
                .map_err(Error::Read)?;
        let request = CollectionPageRequest::new(page, limits)
            .with_stored_calculations(self.stored_calculations || self.collections);
        let mut budget = ResourceBudget::new(
            self.native_bytes.unwrap_or(native::DEFAULT_NATIVE_BYTES),
            row_allowance,
            self.native_steps.unwrap_or(native::DEFAULT_NATIVE_STEPS),
        );
        if let Some(package) =
            native::open_template(&self.path, &mut budget).map_err(Error::Template)?
        {
            return super::template::rows(&package, &self, &mut budget, format);
        }
        let source = native::open(&self.path, password, &mut budget).map_err(Error::Input)?;
        let mut browser = NativeBrowser::new(source, native::CATALOG_SCOPE, budget, &|| false);
        if self.all {
            all::run(&mut browser, &self, request, format)
        } else {
            let table = selected_table(&mut browser, self.table.as_deref())?;
            let text = read_page(&mut browser, &table, request, &self)?;
            format.render(text).map_err(Error::Output)
        }
    }
}

fn read_page<R: tablecraft_ace::NativeReadSource>(
    browser: &mut NativeBrowser<'_, R>,
    table: &str,
    request: CollectionPageRequest,
    args: &Args,
) -> Result<String, Error> {
    if args.multivalues {
        let page = browser
            .multivalue_page_with_request(table, request)
            .map_err(Error::Read)?;
        return multivalue::render(&page);
    }
    match browser.browse_page(table, request).map_err(Error::Read)? {
        BrowsePage::Scalar(page) => display_rows(
            page.columns().iter().map(|c| (c.name(), false)),
            page.rows(),
        ),
        BrowsePage::Collections(page) => collections::render(&page),
    }
}

fn selected_table<R: tablecraft_ace::NativeReadSource>(
    browser: &mut NativeBrowser<'_, R>,
    supplied: Option<&str>,
) -> Result<String, Error> {
    if let Some(name) = supplied {
        return Ok(name.to_owned());
    }

    let inventory = browser.tables().map_err(Error::Read)?;
    let names = inventory
        .entries()
        .filter(|entry| !entry.is_system())
        .map(|entry| entry.object().name().as_str());
    sole_user_table(names).map(str::to_owned)
}

pub(in crate::commands) fn sole_user_table<'a>(
    mut names: impl Iterator<Item = &'a str>,
) -> Result<&'a str, Error> {
    let name = names.next().ok_or(Error::NoUserTables)?;
    if names.next().is_some() {
        return Err(Error::AmbiguousUserTable);
    }
    Ok(name)
}

pub(super) fn display_rows<'a>(
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
    append_rows(&mut output, rows)?;
    Ok(output.finish())
}

pub(super) fn append_rows(output: &mut TableOutput, rows: &[Vec<Value>]) -> Result<(), Error> {
    for row in rows {
        for (index, value) in row.iter().enumerate() {
            if index != 0 {
                output.separator().map_err(Error::Output)?;
            }
            output.value(value).map_err(Error::Output)?;
        }
        output.newline().map_err(Error::Output)?;
    }
    Ok(())
}
