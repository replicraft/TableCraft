use crate::native;
mod attachments;
mod calculated;
mod check;
mod collections;
mod complex;
mod complex_links;
mod constraints;
mod extra;
mod features;
mod history;
mod properties;
mod relationships;
mod saved_query;
mod template;
mod text_order;
use super::error::Error;
use crate::output::TableOutput;
mod args;
pub(crate) use args::Args;
use std::path::Path;
use tablecraft_ace::{Header, InspectError, inspect_catalog, inspect_header};
use tablecraft_core::limits::ResourceBudget;

pub(crate) enum Mode<'a> {
    Header,
    Catalog { show_system: bool },
    Relationships,
    SavedQuery(&'a str),
    Table(&'a str),
    Indexes(&'a str),
    Properties(&'a str),
    Extra(&'a str),
    Features(&'a str),
    Complex(&'a str),
    StoredCalculation(&'a str, &'a str),
    ComplexLinks(&'a str, &'a str),
    Attachments(&'a str, &'a str),
    VersionHistory(&'a str, &'a str),
    Check(&'a str),
    Constraints(&'a str),
}

pub(crate) fn run(
    path: &Path,
    mode: Mode<'_>,
    format: crate::presentation::Format,
    password: Option<&str>,
    native_bytes: Option<u64>,
    native_steps: Option<u64>,
) -> Result<String, Error> {
    let mut budget = ResourceBudget::new(
        native_bytes.unwrap_or(native::DEFAULT_NATIVE_BYTES),
        native::DEFAULT_NATIVE_ROWS,
        native_steps.unwrap_or(native::DEFAULT_NATIVE_STEPS),
    );
    if let Some(report) = template::try_run(path, &mode, format, &mut budget)? {
        return Ok(report);
    }
    let mut source = if matches!(&mode, Mode::Header) {
        native::open_raw(path, &mut budget).map_err(Error::Input)?
    } else {
        native::open(path, password, &mut budget).map_err(Error::Input)?
    };
    let mut output = TableOutput::new();
    if source.is_deployment() {
        line(&mut output, &["Container", "Access deployment (CAB)"])?;
        line(&mut output, &["Package signature", "Not verified"])?;
    }
    match mode {
        Mode::Attachments(table, column) => {
            attachments::run(&mut source, table, column, &mut output, budget)?
        }
        Mode::VersionHistory(table, memo) => {
            history::run(&mut source, table, memo, &mut output, budget)?
        }
        Mode::SavedQuery(name) => saved_query::run(&mut source, name, &mut output, budget)?,
        Mode::ComplexLinks(table, column) => {
            complex_links::run(&mut source, table, column, &mut output, budget)?
        }
        Mode::StoredCalculation(table, column) => {
            calculated::run(&mut source, table, column, &mut output, budget)?
        }
        Mode::Complex(name) => complex::run(&mut source, name, &mut output, budget)?,
        Mode::Features(name) => features::run(&mut source, name, &mut output, budget)?,
        Mode::Relationships => {
            relationships::run(&mut source, &mut output, budget)?;
        }
        Mode::Constraints(name) => {
            constraints::run(&mut source, name, &mut output, budget)?;
        }
        Mode::Check(name) => {
            check::run(&mut source, name, &mut output, budget)?;
        }
        Mode::Properties(name) => {
            properties::run(&mut source, name, &mut output, budget)?;
        }
        Mode::Extra(name) => {
            extra::run(&mut source, name, &mut output, budget)?;
        }
        Mode::Indexes(name) => {
            let inspection = tablecraft_ace::inspect_table_indexes(
                &mut source,
                native::CATALOG_SCOPE,
                name,
                &mut budget,
                &|| false,
            )
            .map_err(Error::Read)?;
            line(&mut output, &["Table", inspection.schema().name()])?;
            output.table_section().map_err(Error::Output)?;
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
        }
        Mode::Table(name) => {
            let mut browser = tablecraft_access::NativeBrowser::new(
                &mut source,
                native::CATALOG_SCOPE,
                budget,
                &|| false,
            );
            let description = browser.describe(name).map_err(Error::Read)?;
            let schema = description.schema();
            line(&mut output, &["Table", schema.name()])?;
            line(
                &mut output,
                &["Declared rows", &schema.declared_rows().to_string()],
            )?;
            line(
                &mut output,
                &[
                    "Scalar columns",
                    if description.scalar_columns_supported() {
                        "Supported carriers; rows not inspected"
                    } else {
                        "Unsupported column carriers"
                    },
                ],
            )?;
            line(
                &mut output,
                &[
                    "Unsupported columns",
                    &description.unsupported_columns().count().to_string(),
                ],
            )?;
            output.table_section().map_err(Error::Output)?;
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
            text_order::columns(schema, &mut output)?;
        }
        Mode::Catalog { show_system } => {
            // Scope is local to this single-file invocation, never a persisted ID.
            let inspection =
                inspect_catalog(&mut source, native::CATALOG_SCOPE, &mut budget, &|| false)
                    .map_err(Error::Read)?;
            header(&mut output, inspection.header())?;
            output.table_section().map_err(Error::Output)?;
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
                if !show_system
                    && object.kind() == tablecraft_model::ObjectKind::Table
                    && origin.is_system()
                {
                    continue;
                }
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
        }
        Mode::Header => {
            let metadata = inspect_header(&mut source, &mut native::header_budget(), &|| false)
                .map_err(Error::Read)?;
            header(&mut output, metadata)?;
            super::encryption::summary(&mut source, &mut output)?;
            text_order::database(&mut source, &mut output, &mut budget)?;
        }
    }
    let (text, sections) = output.finish_sections();
    format.render_report(text, &sections).map_err(Error::Output)
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
