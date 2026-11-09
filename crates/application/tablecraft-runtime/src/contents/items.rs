//! Projection through actual typed item accessors, retaining nullable metadata.
use super::CollectionKind;
use crate::{Cell, Error, display::Presentation};
use tablecraft_ace::InspectError;
use tablecraft_ace::StoredCollectionItem;
use tablecraft_core::values::{LegacyDateTime, Value};

pub(super) fn headers(kind: CollectionKind) -> &'static [&'static str] {
    match kind {
        CollectionKind::Multivalue => &["Value"],
        CollectionKind::Attachments => &[
            "Stored child ID",
            "Name",
            "Type",
            "URL",
            "Modified (rounded civil)",
            "Modified (stored DATE bits)",
            "Flags",
            "Encoded bytes",
        ],
        CollectionKind::VersionHistory => &[
            "Stored child ID",
            "Memo",
            "Modified (rounded civil)",
            "Modified (stored DATE bits)",
        ],
    }
}

pub(super) fn row(
    kind: CollectionKind,
    item: StoredCollectionItem<'_>,
    output: &mut Presentation<'_>,
) -> Result<Vec<Cell>, Error> {
    let mut row = output.row(headers(kind).len())?;
    match (kind, item) {
        (CollectionKind::Multivalue, StoredCollectionItem::Value(value)) => {
            row.push(output.scalar(value, false)?);
        }
        (CollectionKind::Attachments, StoredCollectionItem::Attachment(item)) => {
            row.push(output.scalar(&Value::Int32(item.identity()), false)?);
            row.push(text(output, item.file_name())?);
            row.push(text(output, item.file_type())?);
            row.push(text(output, item.file_url())?);
            let (civil, bits) = date(output, item.file_timestamp())?;
            row.push(civil);
            row.push(bits);
            row.push(output.scalar(&item.file_flags().map_or(Value::Null, Value::Int32), false)?);
            let length = item
                .encoded_file_data()
                .map(|data| {
                    i64::try_from(data.len())
                        .map(Value::Int64)
                        .map_err(|_| InspectError::StructureLimit)
                })
                .transpose()?
                .unwrap_or(Value::Null);
            row.push(output.scalar(&length, false)?);
        }
        (CollectionKind::VersionHistory, StoredCollectionItem::Version(item)) => {
            row.push(output.scalar(&Value::Int32(item.identity()), false)?);
            row.push(text(output, item.value())?);
            let (civil, bits) = date(output, item.modified())?;
            row.push(civil);
            row.push(bits);
        }
        _ => return Err(InspectError::Malformed.into()),
    }
    Ok(row)
}

fn text(output: &mut Presentation<'_>, value: Option<&str>) -> Result<Cell, Error> {
    match value {
        Some(value) => Ok(Cell::expression(output.text(value)?)),
        None => output.scalar(&Value::Null, false),
    }
}

fn date(
    output: &mut Presentation<'_>,
    value: Option<LegacyDateTime>,
) -> Result<(Cell, Cell), Error> {
    output.work(2)?;
    let Some(value) = value else {
        return Ok((
            output.scalar(&Value::Null, false)?,
            output.scalar(&Value::Null, false)?,
        ));
    };
    let civil = match output.scalar(&Value::DateTime(value), false) {
        Ok(value) => value,
        Err(Error::Value(_)) => {
            Cell::expression(output.text("Civil display unavailable (stored DATE retained)")?)
        }
        Err(error) => return Err(error),
    };
    let bits = Cell::expression(output.text(&format!("0x{:016x}", value.to_bits()))?);
    Ok((civil, bits))
}
