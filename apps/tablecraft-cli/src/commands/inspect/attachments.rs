use super::{Error, collections, line};
use crate::{native, output::TableOutput};
use tablecraft_access::NativeBrowser;
use tablecraft_core::values::Value;

pub(super) fn run(
    source: &mut impl tablecraft_ace::NativeReadSource,
    table: &str,
    column: &str,
    output: &mut TableOutput,
    budget: tablecraft_core::limits::ResourceBudget,
) -> Result<(), Error> {
    let mut browser = NativeBrowser::new(source, native::CATALOG_SCOPE, budget, &|| false);
    let stored = browser
        .attachment_column(table, column)
        .map_err(Error::Read)?;
    let links = stored.links();
    let field = links
        .schema()
        .owner()
        .columns()
        .iter()
        .find(|c| c.number() == links.column().number())
        .ok_or(Error::Read(tablecraft_ace::InspectError::Malformed))?;
    collections::summary(output, links, field.name(), "Stored attachments")?;
    line(
        output,
        &[
            "Parent ordinal",
            "Identity",
            "File URL",
            "File name",
            "File type",
            "Encoded bytes",
            "Timestamp",
            "Timestamp bits",
            "Flags",
        ],
    )?;
    for parent in 0..links.parents().len() {
        for attachment in stored
            .attachments(parent)
            .ok_or(Error::Read(tablecraft_ace::InspectError::Malformed))?
        {
            let attachment = attachment.map_err(Error::Read)?;
            output.display(&parent).map_err(Error::Output)?;
            output.separator().map_err(Error::Output)?;
            output
                .display(&attachment.identity())
                .map_err(Error::Output)?;
            collections::text(output, attachment.file_url())?;
            collections::text(output, attachment.file_name())?;
            collections::text(output, attachment.file_type())?;
            output.separator().map_err(Error::Output)?;
            if let Some(data) = attachment.encoded_file_data() {
                output.display(&data.len()).map_err(Error::Output)?;
            } else {
                output.value(&Value::Null).map_err(Error::Output)?;
            }
            collections::timestamp(output, attachment.file_timestamp())?;
            output.separator().map_err(Error::Output)?;
            output
                .value(&attachment.file_flags().map_or(Value::Null, Value::Int32))
                .map_err(Error::Output)?;
            output.newline().map_err(Error::Output)?;
        }
    }
    Ok(())
}
