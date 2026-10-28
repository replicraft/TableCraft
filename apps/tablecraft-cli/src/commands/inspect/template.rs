//! Presentation of the Access backend's inert template package inventory.
use super::{Mode, line};
use crate::{commands::error::Error, output::TableOutput, presentation::Format};
use std::path::Path;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_formats::template::TemplateError;

pub(super) fn try_run(
    path: &Path,
    mode: &Mode<'_>,
    format: Format,
    budget: &mut ResourceBudget,
) -> Result<Option<String>, Error> {
    let Some(package) = crate::native::open_template(path, budget).map_err(Error::Template)? else {
        return Ok(None);
    };
    match mode {
        Mode::Catalog { .. } => {
            return super::super::template::catalog(&package, budget, format).map(Some);
        }
        Mode::Table(name) => {
            return super::super::template::columns(&package, name, budget, format).map(Some);
        }
        Mode::Header => {}
        _ => return Err(Error::Template(TemplateError::Unsupported)),
    }
    let mut output = TableOutput::new();
    for (label, value) in [
        ("Container", "Access template (OPC ZIP)".to_owned()),
        ("Package signature", "Not verified".to_owned()),
        ("Content parts", package.parts().len().to_string()),
        ("Template metadata", package.metadata().name().to_owned()),
        ("Table reading", "Bounded inline XSD data".to_owned()),
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
