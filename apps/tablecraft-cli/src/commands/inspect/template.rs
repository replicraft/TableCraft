//! Presentation of the Access backend's inert template package inventory.
use super::{Mode, line};
use crate::{commands::error::Error, output::TableOutput, presentation::Format};
use std::fs::File;
use std::path::Path;
use tablecraft_access::template::{TemplateError, TemplatePackage};
use tablecraft_core::limits::{Resource, ResourceBudget};

const PREFIX_BYTES: usize = size_of::<u32>();
const ZIP_SIGNATURES: [[u8; PREFIX_BYTES]; 3] = [*b"PK\x03\x04", *b"PK\x05\x06", *b"PK\x07\x08"];

pub(super) fn try_run(
    path: &Path,
    mode: &Mode<'_>,
    format: Format,
    budget: &mut ResourceBudget,
) -> Result<Option<String>, Error> {
    let mut source = File::open(path).map_err(|error| Error::Read(error.into()))?;
    budget
        .charge(Resource::Bytes, PREFIX_BYTES as u64)
        .map_err(|error| Error::Read(error.into()))?;
    let mut prefix = [0; PREFIX_BYTES];
    tablecraft_io::read_exact_at(&mut source, 0, &mut prefix, budget, &|| false)
        .map_err(|error| Error::Read(error.into()))?;
    if !ZIP_SIGNATURES.contains(&prefix) {
        return Ok(None);
    }
    if !matches!(mode, Mode::Header) {
        return Err(Error::Template(TemplateError::Unsupported));
    }
    let package = TemplatePackage::open(&mut source, budget, &|| false).map_err(Error::Template)?;
    let mut output = TableOutput::new();
    for (label, value) in [
        ("Container", "Access template (OPC ZIP)".to_owned()),
        ("Package signature", "Not verified".to_owned()),
        ("Content parts", package.parts().len().to_string()),
        ("Template metadata", package.metadata().name().to_owned()),
        ("Table reading", "Not implemented".to_owned()),
    ] {
        line(&mut output, &[label, &value])?;
    }
    output.table_section().map_err(Error::Output)?;
    line(
        &mut output,
        &["Part", "Content type", "Bytes", "Relationships"],
    )?;
    for part in package.parts() {
        line(
            &mut output,
            &[
                part.name(),
                part.content_type(),
                &part.bytes().len().to_string(),
                &part.relationships().len().to_string(),
            ],
        )?;
    }
    let (text, sections) = output.finish_sections();
    format
        .render_report(text, &sections)
        .map(Some)
        .map_err(Error::Output)
}
