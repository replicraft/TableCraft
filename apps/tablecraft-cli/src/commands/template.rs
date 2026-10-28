//! Template presentation only; package/schema/value interpretation stays in Formats.
use super::{error::Error, rows::Args};
use crate::{output::TableOutput, presentation::Format};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_formats::template::{TemplateError, TemplatePackage};
use tablecraft_model::ObjectKind;

fn line(output: &mut TableOutput, values: &[&str]) -> Result<(), Error> {
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            output.separator().map_err(Error::Output)?;
        }
        output
            .header(Some(value), index + 1)
            .map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)
}

pub(super) fn catalog(
    package: &TemplatePackage,
    budget: &mut ResourceBudget,
    format: Format,
) -> Result<String, Error> {
    let objects = package
        .objects(budget, &|| false)
        .map_err(Error::Template)?;
    let mut output = TableOutput::new();
    line(
        &mut output,
        &["Name", "Kind", "Storage", "Part", "Stored data"],
    )?;
    for object in objects {
        let kind = if object.kind() == ObjectKind::Unknown {
            format!("Unknown ({})", object.declared_type())
        } else {
            format!("{:?}", object.kind())
        };
        line(
            &mut output,
            &[
                object.name(),
                &kind,
                if object.kind() == ObjectKind::Unknown {
                    "Unknown"
                } else if object.is_linked() {
                    "Linked"
                } else {
                    "Template"
                },
                object.part(),
                object.data_part().unwrap_or("Unavailable"),
            ],
        )?;
    }
    format.render(output.finish()).map_err(Error::Output)
}
pub(super) fn columns(
    package: &TemplatePackage,
    name: &str,
    budget: &mut ResourceBudget,
    format: Format,
) -> Result<String, Error> {
    let columns = package
        .table_columns(name, budget, &|| false)
        .map_err(Error::Template)?;
    let mut output = TableOutput::new();
    line(&mut output, &["Column", "Inline type", "Nullable"])?;
    for column in columns {
        line(
            &mut output,
            &[
                column.name(),
                &format!("{:?}", column.data_type()),
                if column.nullable() { "TRUE" } else { "FALSE" },
            ],
        )?;
    }
    format.render(output.finish()).map_err(Error::Output)
}
pub(super) fn tables(
    package: &TemplatePackage,
    details: bool,
    count: bool,
    budget: &mut ResourceBudget,
) -> Result<String, Error> {
    let objects = package
        .objects(budget, &|| false)
        .map_err(Error::Template)?;
    let mut output = TableOutput::new();
    let mut headers = vec!["Name", "Kind"];
    if count {
        headers.push("Row Count");
    }
    if details {
        headers.extend(["Storage", "Rows"]);
    }
    line(&mut output, &headers)?;
    for object in objects
        .iter()
        .filter(|object| object.kind() == ObjectKind::Table)
    {
        let count_text = if count && !object.is_linked() && object.data_part().is_some() {
            package
                .read_table(object.name(), budget, &|| false)
                .map_err(Error::Template)?
                .rows()
                .len()
                .to_string()
        } else {
            "N/A".into()
        };
        let mut values = vec![object.name(), "Template"];
        if count {
            values.push(&count_text);
        }
        if details {
            values.extend([
                if object.is_linked() {
                    "Linked"
                } else {
                    "Stored template"
                },
                if object.is_linked() {
                    "External source not opened"
                } else {
                    "Stored data; explicit read required"
                },
            ]);
        }
        line(&mut output, &values)?;
    }
    Ok(output.finish())
}
pub(super) fn rows(
    package: &TemplatePackage,
    args: &Args,
    budget: &mut ResourceBudget,
    format: Format,
) -> Result<String, Error> {
    if args.multivalues || args.collections || args.stored_calculations {
        return Err(Error::Template(TemplateError::Unsupported));
    }
    let objects = package
        .objects(budget, &|| false)
        .map_err(Error::Template)?;
    let selected: Vec<_> = objects
        .iter()
        .filter(|object| {
            object.kind() == ObjectKind::Table
                && (args.all
                    || args
                        .table
                        .as_ref()
                        .is_some_and(|name| object.name().eq_ignore_ascii_case(name)))
        })
        .collect();
    if !args.all && selected.is_empty() {
        return Err(Error::Template(TemplateError::Unsupported));
    }
    let mut output = TableOutput::new();
    for (index, object) in selected.into_iter().enumerate() {
        let text = if object.is_linked() || object.data_part().is_none() {
            if !args.all {
                return Err(Error::Template(TemplateError::Unsupported));
            }
            "Status\nExternal or missing stored data not opened\n".into()
        } else {
            let table = package
                .read_table(object.name(), budget, &|| false)
                .map_err(Error::Template)?;
            let start = (args.offset as usize).min(table.rows().len());
            let end = start
                .saturating_add(args.limit as usize)
                .min(table.rows().len());
            super::rows::display_rows(
                table.columns().iter().map(|column| (column.name(), false)),
                &table.rows()[start..end],
            )?
        };
        let rendered = if args.all {
            let mut section = TableOutput::new();
            line(&mut section, &["Table", object.name()])?;
            section.table_section().map_err(Error::Output)?;
            section.display(&text).map_err(Error::Output)?;
            let (text, sections) = section.finish_sections();
            format
                .render_report(text, &sections)
                .map_err(Error::Output)?
        } else {
            format.render(text).map_err(Error::Output)?
        };
        if index != 0 {
            output.newline().map_err(Error::Output)?;
        }
        output.display(&rendered).map_err(Error::Output)?;
    }
    Ok(output.finish())
}
