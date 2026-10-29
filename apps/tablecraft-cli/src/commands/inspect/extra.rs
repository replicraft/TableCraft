use super::{Error, line};
use crate::{native, output::TableOutput};
use std::io::{Read, Seek};
use tablecraft_ace::{InspectError, NativeImage, PayloadPresence, TableExtraCoverage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::ObjectKind;

pub(super) fn run<R: Read + Seek>(
    source: &mut R,
    name: &str,
    output: &mut TableOutput,
    mut budget: ResourceBudget,
) -> Result<(), Error> {
    let image = NativeImage::capture(source, native::CATALOG_SCOPE, &mut budget, &|| false)
        .map_err(Error::Read)?;
    let object = image
        .catalog()
        .catalog()
        .lookup(name, Some(ObjectKind::Table), None)
        .map_err(InspectError::from)
        .map_err(Error::Read)?
        .ok_or(Error::Read(InspectError::UnknownTable))?;
    let extra = image
        .inspect_table_extra(object.id(), &mut budget, &|| false)
        .map_err(Error::Read)?;

    line(output, &["Table", object.name().as_str()])?;
    line(output, &["Native ID", &extra.native_id().to_string()])?;
    line(
        output,
        &["Native parent ID", &extra.parent_id().to_string()],
    )?;
    line(output, &["Catalog page", &extra.catalog_page().to_string()])?;
    line(output, &["Catalog slot", &extra.catalog_slot().to_string()])?;
    let presence = match extra.presence() {
        PayloadPresence::Uninspected => "Column missing",
        PayloadPresence::Absent => "NULL",
        PayloadPresence::Present => "Present",
    };
    line(output, &["LvExtra field", presence])?;
    let coverage = match extra.coverage() {
        TableExtraCoverage::NotPresent => "No payload inspected",
        TableExtraCoverage::Empty => "Present empty payload",
        TableExtraCoverage::WideMr2 => "Observed wide-MR2 framing",
        TableExtraCoverage::Opaque => "Opaque content retained",
    };
    line(output, &["Structural coverage", coverage])?;
    if let Some(raw) = extra.raw() {
        line(output, &["Payload bytes", &raw.len().to_string()])?;
    }
    line(
        output,
        &["Property names", &extra.names().len().to_string()],
    )?;
    line(output, &["Value groups", &extra.groups().len().to_string()])?;
    line(output, &["Data-macro absence", "Not certified"])?;

    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &[
            "Group kind",
            "Group body bytes",
            "Property",
            "Flags",
            "Native type",
            "Value bytes",
            "Validated text",
        ],
    )?;
    for group in extra.groups() {
        match group.properties() {
            Some(properties) if !properties.is_empty() => {
                for property in properties {
                    line(
                        output,
                        &[
                            &group.kind().to_string(),
                            &group.raw().len().to_string(),
                            property.name(),
                            &format!("0x{:02x}", property.flags()),
                            &property.native_type().to_string(),
                            &property.raw().len().to_string(),
                            if property.text().is_some() {
                                "TRUE"
                            } else {
                                "FALSE"
                            },
                        ],
                    )?;
                }
            }
            properties => {
                line(
                    output,
                    &[
                        &group.kind().to_string(),
                        &group.raw().len().to_string(),
                        if properties.is_some() {
                            "EMPTY"
                        } else {
                            "OPAQUE"
                        },
                        "",
                        "",
                        "",
                        "",
                    ],
                )?;
            }
        }
    }
    Ok(())
}
