use super::{Error, line};
use crate::{native, output::TableOutput};
use std::fs::File;
use tablecraft_core::values::Value;

pub(super) fn run(source: &mut File, name: &str, output: &mut TableOutput) -> Result<(), Error> {
    let definition = tablecraft_ace::read_saved_query_definition(
        source,
        native::CATALOG_SCOPE,
        name,
        &mut native::budget(),
        &|| false,
    )
    .map_err(Error::Read)?;
    line(output, &["Query", definition.object().name().as_str()])?;
    line(
        output,
        &["Native ID", &definition.origin().native_id().to_string()],
    )?;
    line(
        output,
        &["Flags", &format!("0x{:08x}", definition.origin().flags())],
    )?;
    line(
        output,
        &["Fragment rows", &definition.rows().len().to_string()],
    )?;
    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &[
            "Attribute",
            "Expression",
            "Flag",
            "LvExtra",
            "Name1",
            "Name2",
            "Order",
        ],
    )?;
    for row in definition.rows() {
        output
            .value(
                &row.attribute()
                    .map_or(Value::Null, |value| Value::Byte(value.code())),
            )
            .map_err(Error::Output)?;
        output.separator().map_err(Error::Output)?;
        text(output, row.expression())?;
        output.separator().map_err(Error::Output)?;
        output
            .value(&row.flag().map_or(Value::Null, Value::Int16))
            .map_err(Error::Output)?;
        output.separator().map_err(Error::Output)?;
        output
            .value(&row.extra().map_or(Value::Null, Value::Int32))
            .map_err(Error::Output)?;
        output.separator().map_err(Error::Output)?;
        text(output, row.name1())?;
        output.separator().map_err(Error::Output)?;
        text(output, row.name2())?;
        output.separator().map_err(Error::Output)?;
        match row.order() {
            None => output.value(&Value::Null),
            Some(bytes) => output.binary_value(bytes),
        }
        .map_err(Error::Output)?;
        output.newline().map_err(Error::Output)?;
    }
    Ok(())
}
fn text(output: &mut TableOutput, value: Option<&str>) -> Result<(), Error> {
    match value {
        None => output.value(&Value::Null),
        Some(value) => output.text_value(value),
    }
    .map_err(Error::Output)
}
