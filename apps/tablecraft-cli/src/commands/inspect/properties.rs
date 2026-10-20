use super::{Error, line};
use crate::{native, output::TableOutput};
use tablecraft_ace::{PropertyGroupKind, inspect_table_properties};

pub(super) fn run<R: tablecraft_ace::NativeReadSource>(
    source: &mut R,
    name: &str,
    output: &mut TableOutput,
    mut budget: tablecraft_core::limits::ResourceBudget,
) -> Result<(), Error> {
    let properties =
        inspect_table_properties(source, native::CATALOG_SCOPE, name, &mut budget, &|| false)
            .map_err(Error::Read)?;
    line(output, &["Table", properties.name()])?;
    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &[
            "Scope",
            "Object",
            "Property",
            "Native type",
            "Flags",
            "Value",
        ],
    )?;
    for group in properties.groups() {
        let scope = match group.kind() {
            PropertyGroupKind::Table => "Table",
            PropertyGroupKind::Column => "Column",
            PropertyGroupKind::Index => "Index",
            PropertyGroupKind::Unknown(_) => "Unknown",
        };
        for property in group.properties() {
            let value = if let Ok(value) = property.boolean() {
                if value { "TRUE" } else { "FALSE" }
            } else {
                property.text().unwrap_or("OPAQUE")
            };
            line(
                output,
                &[
                    scope,
                    group.name(),
                    property.name(),
                    &property.native_type().to_string(),
                    &format!("0x{:02x}", property.flags()),
                    value,
                ],
            )?;
        }
    }
    Ok(())
}
