use super::{Error, line};
use crate::{native, output::TableOutput};
use std::io::{Read, Seek};
use tablecraft_ace::inspect_table_features;

pub(super) fn run<R: Read + Seek>(
    source: &mut R,
    name: &str,
    output: &mut TableOutput,
) -> Result<(), Error> {
    let features = inspect_table_features(
        source,
        native::CATALOG_SCOPE,
        name,
        &mut native::budget(),
        &|| false,
    )
    .map_err(Error::Read)?;
    line(output, &["Table", features.schema().name()])?;
    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &[
            "Column",
            "Calculated",
            "Complex type ID",
            "Complex FK",
            "AutoNumber",
            "Hyperlink",
            "Append only",
            "Text format",
            "Result type",
            "Expression",
        ],
    )?;
    for field in features.columns() {
        let optional = |value: Option<u8>| {
            value
                .map(|v| v.to_string())
                .unwrap_or_else(|| "UNKNOWN".into())
        };
        let flag = |value: bool| if value { "TRUE" } else { "FALSE" };
        let append = field
            .append_only()
            .map_err(Error::Read)?
            .map(flag)
            .unwrap_or("UNKNOWN");
        line(
            output,
            &[
                field.column().name(),
                flag(field.calculated()),
                &field
                    .column()
                    .complex_type_id()
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "NONE".into()),
                flag(field.complex_foreign_key()),
                flag(field.auto_number()),
                flag(field.hyperlink()),
                append,
                &optional(field.text_format().map_err(Error::Read)?),
                &optional(field.result_native_type().map_err(Error::Read)?),
                field.expression().map_err(Error::Read)?.unwrap_or("NONE"),
            ],
        )?;
    }
    Ok(())
}
