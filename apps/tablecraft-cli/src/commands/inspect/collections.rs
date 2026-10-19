//! Common presentation of checked stored collection membership and nullable fields.
use super::{Error, line};
use crate::output::TableOutput;
use tablecraft_ace::ComplexColumnLinks;
use tablecraft_core::values::{LegacyDateTime, Value};

pub(super) fn summary(
    output: &mut TableOutput,
    links: &ComplexColumnLinks,
    column: &str,
    kind: &str,
) -> Result<(), Error> {
    line(output, &["Table", links.schema().owner().name()])?;
    line(output, &["Column", column])?;
    line(output, &["Parent rows", &links.parents().len().to_string()])?;
    line(
        output,
        &[kind, &links.child_snapshot().rows().len().to_string()],
    )?;
    output.table_section().map_err(Error::Output)?;
    line(output, &["Parent ordinal", "Complex key", kind])?;
    for (ordinal, parent) in links.parents().iter().enumerate() {
        output.display(&ordinal).map_err(Error::Output)?;
        output.separator().map_err(Error::Output)?;
        output
            .value(&parent.key().map_or(Value::Null, Value::Int32))
            .map_err(Error::Output)?;
        output.separator().map_err(Error::Output)?;
        output
            .display(&parent.child_count())
            .map_err(Error::Output)?;
        output.newline().map_err(Error::Output)?;
    }
    output.table_section().map_err(Error::Output)
}

pub(super) fn text(output: &mut TableOutput, value: Option<&str>) -> Result<(), Error> {
    output.separator().map_err(Error::Output)?;
    match value {
        Some(value) => output.text_value(value),
        None => output.value(&Value::Null),
    }
    .map_err(Error::Output)
}

pub(super) fn timestamp(
    output: &mut TableOutput,
    value: Option<LegacyDateTime>,
) -> Result<(), Error> {
    output.separator().map_err(Error::Output)?;
    output
        .value(&value.map_or(Value::Null, Value::DateTime))
        .map_err(Error::Output)?;
    output.separator().map_err(Error::Output)?;
    if let Some(value) = value {
        output
            .display(&format_args!("0x{:016x}", value.to_bits()))
            .map_err(Error::Output)
    } else {
        output.value(&Value::Null).map_err(Error::Output)
    }
}
