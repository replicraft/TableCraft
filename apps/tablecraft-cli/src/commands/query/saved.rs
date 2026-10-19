use super::Error;
use crate::{native, output::TableOutput};
use std::{fs::File, path::Path};

pub(super) fn run(path: &Path, name: &str) -> Result<String, Error> {
    let _span = tracing::debug_span!("cli.query.saved").entered();
    let mut source = File::open(path).map_err(|error| Error::Native(error.into()))?;
    let mut budget = native::budget();
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
