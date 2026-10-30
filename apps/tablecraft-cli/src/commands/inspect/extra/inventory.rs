//! Explicit inert envelope observations; no program body text or native dispatch inference.
use super::{Error, line};
use crate::output::TableOutput;
use tablecraft_ace::{InspectError, MacroPropertyObservation, TableExtra};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn write(
    extra: &TableExtra,
    output: &mut TableOutput,
    budget: &mut ResourceBudget,
) -> Result<(), Error> {
    let inventory = extra
        .inspect_macro_inventory(budget, &|| false)
        .map_err(Error::Read)?;
    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &[
            "Group ordinal",
            "Group kind",
            "Property ordinal",
            "Property",
            "Observation",
            "XML namespace",
            "Document ordinal",
            "Event",
            "Macro name",
        ],
    )?;
    for entry in inventory.properties() {
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)
            .map_err(Error::Read)?;
        let group = &extra.groups()[entry.group_ordinal()];
        let property = &group.properties().unwrap()[entry.property_ordinal()];
        let group_ordinal = entry.group_ordinal().to_string();
        let kind = group.kind().to_string();
        let property_ordinal = entry.property_ordinal().to_string();
        let prefix = [
            group_ordinal.as_str(),
            kind.as_str(),
            property_ordinal.as_str(),
            property.name(),
        ];
        match entry.observation() {
            MacroPropertyObservation::Envelopes(xml) => {
                if xml.definitions().is_empty() {
                    line(
                        output,
                        &[
                            prefix[0],
                            prefix[1],
                            prefix[2],
                            prefix[3],
                            "No envelopes observed",
                            xml.namespace().uri(),
                            "",
                            "",
                            "",
                        ],
                    )?;
                }
                for (ordinal, definition) in xml.definitions().iter().enumerate() {
                    budget
                        .charge(Resource::Steps, 1)
                        .map_err(InspectError::from)
                        .map_err(Error::Read)?;
                    line(
                        output,
                        &[
                            prefix[0],
                            prefix[1],
                            prefix[2],
                            prefix[3],
                            "Observed envelope",
                            xml.namespace().uri(),
                            &ordinal.to_string(),
                            definition.event().unwrap_or(""),
                            definition.name().unwrap_or(""),
                        ],
                    )?;
                }
            }
            observation => {
                let status = match observation {
                    MacroPropertyObservation::NoText => "Unresolved: no qualified text",
                    MacroPropertyObservation::UnsupportedXml => {
                        "Unresolved: unsupported XML profile"
                    }
                    MacroPropertyObservation::MalformedXml => "Unresolved: malformed XML",
                    MacroPropertyObservation::Envelopes(_) => unreachable!(),
                };
                line(
                    output,
                    &[
                        prefix[0], prefix[1], prefix[2], prefix[3], status, "", "", "", "",
                    ],
                )?;
            }
        }
    }
    for &ordinal in inventory.opaque_groups() {
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)
            .map_err(Error::Read)?;
        line(
            output,
            &[
                &ordinal.to_string(),
                &extra.groups()[ordinal].kind().to_string(),
                "",
                "",
                "Unresolved: opaque group",
                "",
                "",
                "",
                "",
            ],
        )?;
    }
    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &["Name ordinal", "Name without parsed property reference"],
    )?;
    for &ordinal in inventory.unrepresented_names() {
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)
            .map_err(Error::Read)?;
        line(output, &[&ordinal.to_string(), &extra.names()[ordinal]])?;
    }
    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &["Inventory scope", "Retained table-owned Extra only"],
    )?;
    line(output, &["Opaque name references", "Unknown"])?;
    line(output, &["Required macro coverage", "Not certified"])?;
    line(output, &["Automatic execution", "Not authorized"])?;
    line(output, &["Save permission", "Not granted"])
}
