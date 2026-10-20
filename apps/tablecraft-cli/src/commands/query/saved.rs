use super::Error;
use crate::{native, output::TableOutput};
use std::path::Path;

pub(super) fn run(path: &Path, name: &str, password: Option<&str>) -> Result<String, Error> {
    let _span = tracing::debug_span!("cli.query.saved").entered();
    let mut budget = native::budget();
    let mut source = native::open(path, password, &mut budget).map_err(Error::Input)?;
    let definition = tablecraft_ace::read_saved_query_definition(
        &mut source,
        native::CATALOG_SCOPE,
        name,
        &mut budget,
        &|| false,
    )
    .map_err(Error::Native)?;
    let saved = tablecraft_access::reconstruct_saved_select(
        definition,
        Default::default(),
        &mut budget,
        &|| false,
    )
    .map_err(Error::Saved)?;
    let mut output = TableOutput::new();
    output.display(saved.query()).map_err(Error::Output)?;
    output.newline().map_err(Error::Output)?;
    Ok(output.finish())
}
