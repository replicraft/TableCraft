use super::{Error, line};
use crate::output::TableOutput;
use std::io::{Read, Seek};
use tablecraft_ace::{
    GeneralSortFamily, InspectError, NativeSortOrder, TableSchema, inspect_database_sort_order,
};
use tablecraft_core::limits::ResourceBudget;

fn family(order: NativeSortOrder) -> &'static str {
    match order.general_family() {
        Some(GeneralSortFamily::Legacy) => "General legacy",
        Some(GeneralSortFamily::General2010) => "General 2010",
        None => "Unqualified",
    }
}
pub(super) fn database<R: Read + Seek>(
    source: &mut R,
    output: &mut TableOutput,
    budget: &mut ResourceBudget,
) -> Result<(), Error> {
    match inspect_database_sort_order(source, budget, &|| false) {
        Ok(order) => {
            line(output, &["Text sort LCID", &order.lcid().to_string()])?;
            line(output, &["Text sort version", &order.version().to_string()])?;
            line(
                output,
                &["Text sort variant", &format!("0x{:02x}", order.variant())],
            )?;
            line(output, &["Text sort family", family(order)])
        }
        Err(InspectError::Unsupported) => line(output, &["Text sort family", "Unqualified"]),
        Err(error) => Err(Error::Read(error)),
    }
}
pub(super) fn columns(schema: &TableSchema, output: &mut TableOutput) -> Result<(), Error> {
    if !schema
        .columns()
        .iter()
        .any(|c| c.text_sort_order().is_some())
    {
        return Ok(());
    }
    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &[
            "Text column",
            "Sort LCID",
            "Sort version",
            "Sort variant",
            "Sort family",
        ],
    )?;
    for column in schema.columns() {
        if let Some(order) = column.text_sort_order() {
            line(
                output,
                &[
                    column.name(),
                    &order.lcid().to_string(),
                    &order.version().to_string(),
                    &format!("0x{:02x}", order.variant()),
                    family(order),
                ],
            )?;
        }
    }
    Ok(())
}
