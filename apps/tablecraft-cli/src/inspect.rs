use crate::native;
mod check;
use crate::output::{OutputError, TableOutput};
use std::{fmt, fs::File, path::Path};
use tablecraft_ace::{Header, InspectError, inspect_catalog, inspect_header, inspect_table_schema};

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
pub(crate) fn run(
    path: &Path,
    catalog: bool,
    table: Option<&str>,
    indexes: bool,
    check: bool,
) -> Result<String, Error> {
    let mut source = File::open(path).map_err(|e| Error::Read(e.into()))?;
    let mut output = TableOutput::new();
    if let Some(name) = table.filter(|_| check) {
        check::run(&mut source, name, &mut output)?;
    } else if let Some(name) = table.filter(|_| indexes) {
        let inspection = tablecraft_ace::inspect_table_indexes(
            &mut source,
            native::CATALOG_SCOPE,
            name,
            &mut native::budget(),
            &|| false,
        )
        .map_err(Error::Read)?;
        line(&mut output, &["Table", inspection.schema().name()])?;
        line(
            &mut output,
            &[
                "Index",
                "Logical",
                "Physical",
                "Root page",
                "Type",
                "Flags",
                "Primary",
                "Unique",
                "Required key",
                "Ignore NULLs",
                "Column",
                "Descending",
            ],
        )?;
        for index in inspection.indexes() {
            let constraints = index.constraints();
            for column in index.columns() {
                let name = inspection
                    .schema()
                    .columns()
                    .iter()
                    .find(|c| c.number() == column.number())
                    .ok_or(Error::Read(InspectError::Malformed))?
                    .name();
                line(
                    &mut output,
                    &[
                        index.name(),
                        &index.number().to_string(),
                        &index.data_number().to_string(),
                        &index.root_page().to_string(),
                        &index.native_type().to_string(),
                        &format!("0x{:02x}", index.flags()),
                        declaration(constraints.map(|c| c.primary_key())),
                        declaration(constraints.map(|c| c.unique())),
                        declaration(constraints.map(|c| c.required())),
                        declaration(constraints.map(|c| c.ignore_nulls())),
                        name,
                        if column.descending() { "TRUE" } else { "FALSE" },
                    ],
                )?;
            }
        }
    } else if let Some(name) = table {
        let schema = inspect_table_schema(
            &mut source,
            native::CATALOG_SCOPE,
            name,
            &mut native::budget(),
            &|| false,
        )
        .map_err(Error::Read)?;
        line(&mut output, &["Table", schema.name()])?;
        line(
            &mut output,
            &["Declared rows", &schema.declared_rows().to_string()],
        )?;
        line(
            &mut output,
            &[
                "Column",
                "Native type",
                "Storage bytes",
                "Flags",
                "Extended flags",
                "Scalar carrier",
            ],
        )?;
        for c in schema.columns() {
            let carrier = c
                .scalar_type()
                .map(|s| format!("{s:?}"))
                .unwrap_or_else(|| "Unsupported".into());
            line(
                &mut output,
                &[
                    c.name(),
                    &c.native_type().to_string(),
                    &c.storage_length().to_string(),
                    &format!("0x{:02x}", c.flags()),
                    &format!("0x{:02x}", c.extended_flags()),
                    &carrier,
                ],
            )?;
        }
    } else if catalog {
        // Scope is local to this single-file invocation, never a persisted ID.
        let inspection = inspect_catalog(
            &mut source,
            native::CATALOG_SCOPE,
            &mut native::budget(),
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
        let metadata = inspect_header(&mut source, &mut native::header_budget(), &|| false)
            .map_err(Error::Read)?;
        header(&mut output, metadata)?;
    }
    Ok(output.finish())
}

fn declaration(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "TRUE",
        Some(false) => "FALSE",
        None => "UNKNOWN",
    }
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
