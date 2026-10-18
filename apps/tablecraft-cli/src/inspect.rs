use crate::output::{OutputError, TableOutput};
use std::{fmt, fs::File, path::Path};
use tablecraft_ace::{Header, InspectError, inspect_catalog, inspect_header};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

pub(crate) enum Error {
    Read(InspectError),
    Output(OutputError),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(e) => e.fmt(f),
            Self::Output(e) => e.fmt(f),
        }
    }
}
pub(crate) fn run(path: &Path, catalog: bool) -> Result<String, Error> {
    let mut source = File::open(path).map_err(|e| Error::Read(e.into()))?;
    let mut output = TableOutput::new();
    if catalog {
        // Scope is local to this single-file invocation, never a persisted ID.
        let inspection = inspect_catalog(
            &mut source,
            CatalogScope::new(1),
            &mut ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
            &|| false,
        )
        .map_err(Error::Read)?;
        header(&mut output, inspection.header())?;
        line(
            &mut output,
            &[
                "Object ID",
                "Kind",
                "Native ID",
                "Parent ID",
                "Native type",
                "Flags",
                "Name",
            ],
        )?;
        for (object, origin) in inspection
            .catalog()
            .objects()
            .iter()
            .zip(inspection.origins())
        {
            line(
                &mut output,
                &[
                    &object.id().as_u128().to_string(),
                    &format!("{:?}", object.kind()),
                    &origin.native_id().to_string(),
                    &origin.parent_id().to_string(),
                    &origin.type_code().to_string(),
                    &format!("0x{:08x}", origin.flags()),
                    object.name().as_str(),
                ],
            )?;
        }
    } else {
        let metadata = inspect_header(&mut source, &mut ResourceBudget::new(66, 0, 10), &|| false)
            .map_err(Error::Read)?;
        header(&mut output, metadata)?;
    }
    Ok(output.finish())
}
fn header(output: &mut TableOutput, header: Header) -> Result<(), Error> {
    for (label, value) in [
        ("Format", format!("{:?}", header.format())),
        ("Subversion", header.subversion().to_string()),
        ("Page bytes", header.page_size().to_string()),
        ("Pages", header.page_count().to_string()),
        ("Encoded pages", header.encoded_pages().to_string()),
    ] {
        line(output, &[label, &value])?;
    }
    Ok(())
}
fn line(output: &mut TableOutput, columns: &[&str]) -> Result<(), Error> {
    for (i, text) in columns.iter().enumerate() {
        if i > 0 {
            output.separator().map_err(Error::Output)?;
        }
        output.header(Some(text), i + 1).map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)
}
