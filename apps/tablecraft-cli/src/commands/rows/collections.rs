use super::super::error::Error;
use crate::output::TableOutput;
use tablecraft_access::{
    StoredCollectionColumn, StoredCollectionItem, StoredCollectionPageValue,
    StoredCollectionTablePage,
};
use tablecraft_core::values::{LegacyDateTime, Value};

pub(super) fn render(page: &StoredCollectionTablePage) -> Result<String, Error> {
    let mut output = TableOutput::new();
    for (index, field) in page.schema().columns().iter().enumerate() {
        if index != 0 {
            output.separator().map_err(Error::Output)?;
        }
        output
            .header(Some(field.name()), index + 1)
            .map_err(Error::Output)?;
        match page.collection_column(index) {
            Some(StoredCollectionColumn::Multivalue(_)) => {
                output.display(&" [multivalue]").map_err(Error::Output)?
            }
            Some(StoredCollectionColumn::Attachments(_)) => {
                output.display(&" [attachments]").map_err(Error::Output)?
            }
            Some(StoredCollectionColumn::VersionHistory(history)) => {
                output.display(&" [history: ").map_err(Error::Output)?;
                output
                    .header(Some(history.memo_column().name()), index + 1)
                    .map_err(Error::Output)?;
                output.display(&"]").map_err(Error::Output)?;
            }
            None => {
                if page
                    .features()
                    .and_then(|f| f.columns().nth(index))
                    .is_some_and(|f| f.calculated())
                {
                    output.display(&" [cached]").map_err(Error::Output)?;
                }
            }
        }
    }
    output.newline().map_err(Error::Output)?;
    for row in 0..page.row_count() {
        for column in 0..page.schema().columns().len() {
            if column != 0 {
                output.separator().map_err(Error::Output)?;
            }
            match page
                .cell(row, column)
                .ok_or(Error::Read(tablecraft_ace::InspectError::Malformed))?
            {
                StoredCollectionPageValue::Scalar(value) => {
                    output.value(value).map_err(Error::Output)?
                }
                StoredCollectionPageValue::Collection(cell) => {
                    if cell.key().is_none() {
                        output.value(&Value::Null).map_err(Error::Output)?;
                        continue;
                    }
                    output.display(&"[").map_err(Error::Output)?;
                    for (index, item) in cell.items().map_err(Error::Read)?.enumerate() {
                        if index != 0 {
                            output.display(&", ").map_err(Error::Output)?;
                        }
                        match item.map_err(Error::Read)? {
                            StoredCollectionItem::Value(value) => {
                                output.value(value).map_err(Error::Output)?
                            }
                            StoredCollectionItem::Attachment(value) => {
                                output
                                    .display(&format_args!("{{id: {}, url: ", value.identity()))
                                    .map_err(Error::Output)?;
                                text(&mut output, value.file_url())?;
                                output.display(&", name: ").map_err(Error::Output)?;
                                text(&mut output, value.file_name())?;
                                output.display(&", type: ").map_err(Error::Output)?;
                                text(&mut output, value.file_type())?;
                                output
                                    .display(&", encoded_bytes: ")
                                    .map_err(Error::Output)?;
                                if let Some(bytes) = value.encoded_file_data() {
                                    output.display(&bytes.len()).map_err(Error::Output)?;
                                } else {
                                    output.value(&Value::Null).map_err(Error::Output)?;
                                }
                                output.display(&", timestamp: ").map_err(Error::Output)?;
                                timestamp(&mut output, value.file_timestamp())?;
                                output.display(&", flags: ").map_err(Error::Output)?;
                                output
                                    .value(&value.file_flags().map_or(Value::Null, Value::Int32))
                                    .map_err(Error::Output)?;
                                output.display(&"}").map_err(Error::Output)?;
                            }
                            StoredCollectionItem::Version(value) => {
                                output
                                    .display(&format_args!("{{id: {}, value: ", value.identity()))
                                    .map_err(Error::Output)?;
                                text(&mut output, value.value())?;
                                output.display(&", modified: ").map_err(Error::Output)?;
                                timestamp(&mut output, value.modified())?;
                                output.display(&"}").map_err(Error::Output)?;
                            }
                        }
                    }
                    output.display(&"]").map_err(Error::Output)?;
                }
            }
        }
        output.newline().map_err(Error::Output)?;
    }
    Ok(output.finish())
}

fn text(output: &mut TableOutput, text: Option<&str>) -> Result<(), Error> {
    match text {
        Some(text) => output.text_value(text),
        None => output.value(&Value::Null),
    }
    .map_err(Error::Output)
}
fn timestamp(output: &mut TableOutput, value: Option<LegacyDateTime>) -> Result<(), Error> {
    output
        .value(&value.map_or(Value::Null, Value::DateTime))
        .map_err(Error::Output)?;
    if let Some(value) = value {
        output
            .display(&format_args!(" (bits=0x{:016x})", value.to_bits()))
            .map_err(Error::Output)?;
    }
    Ok(())
}
