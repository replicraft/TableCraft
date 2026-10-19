use crate::{inspect::Error, output::TableOutput};
use tablecraft_core::values::Value;

pub(super) fn render(page: &tablecraft_access::MultivalueTablePage) -> Result<String, Error> {
    use tablecraft_access::MultivaluePageValue;
    let mut output = TableOutput::new();
    for (index, field) in page.schema().columns().iter().enumerate() {
        if index != 0 {
            output.separator().map_err(Error::Output)?;
        }
        output
            .header(Some(field.name()), index + 1)
            .map_err(Error::Output)?;
        if page.collection_column(index).is_some() {
            output.display(&" [multivalue]").map_err(Error::Output)?;
        } else if page
            .features()
            .and_then(|f| f.columns().nth(index))
            .is_some_and(|f| f.calculated())
        {
            output.display(&" [cached]").map_err(Error::Output)?;
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
                MultivaluePageValue::Scalar(value) => output.value(value).map_err(Error::Output)?,
                MultivaluePageValue::Collection(cell) => {
                    if cell.key().is_none() {
                        output.value(&Value::Null).map_err(Error::Output)?;
                        continue;
                    }
                    output.display(&"[").map_err(Error::Output)?;
                    for (index, value) in cell.values().map_err(Error::Read)?.enumerate() {
                        if index != 0 {
                            output.display(&", ").map_err(Error::Output)?;
                        }
                        output.value(value).map_err(Error::Output)?;
                    }
                    output.display(&"]").map_err(Error::Output)?;
                }
            }
        }
        output.newline().map_err(Error::Output)?;
    }
    Ok(output.finish())
}
