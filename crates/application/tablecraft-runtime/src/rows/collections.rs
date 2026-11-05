//! Complete typed collection summaries; no attachment decoding or child actions.
use crate::{Cell, CellKind, Error, HorizontalAlignment, display::Presentation};
use tablecraft_access::{
    StoredCollectionColumn, StoredCollectionItem, StoredCollectionPageValue,
    StoredCollectionTablePage,
};
use tablecraft_ace::InspectError;

pub(super) fn rows(
    page: &StoredCollectionTablePage,
    output: &mut Presentation<'_>,
) -> Result<Vec<Vec<Cell>>, Error> {
    let columns = page.schema().columns();
    let mut cached = output.flags(columns.len())?;
    if let Some(features) = page.features() {
        // columns() joins each native field to its property group. Prepay the
        // complete possible name-comparison work, including unmatched groups.
        let groups = features.properties().groups();
        let mut group_bytes = 0_u64;
        for group in groups {
            output.work(1)?;
            group_bytes = group_bytes
                .checked_add(group.name().len() as u64)
                .ok_or(InspectError::StructureLimit)?;
        }
        for native in columns {
            output.work(1)?;
            let names = (native.name().len() as u64)
                .checked_mul(groups.len() as u64)
                .and_then(|bytes| bytes.checked_add(group_bytes))
                .and_then(|bytes| bytes.checked_add(groups.len() as u64))
                .ok_or(InspectError::StructureLimit)?;
            output.work(names)?;
        }
        for field in features.columns() {
            output.work(1)?;
            cached.push(field.calculated());
        }
    } else {
        for _ in columns {
            output.work(1)?;
            cached.push(false);
        }
    }
    if cached.len() != columns.len() {
        return Err(InspectError::Malformed.into());
    }
    let mut rows = output.rows(page.row_count())?;
    for parent in 0..page.row_count() {
        let mut row = output.row(columns.len())?;
        for (column, stored_calculation) in cached.iter().copied().enumerate() {
            // cell() performs a bounded collection-column lookup. Parent is the
            // page-local ordinal; the engine applies its checked request offset.
            output.work(columns.len() as u64)?;
            let value = page.cell(parent, column).ok_or(InspectError::Malformed)?;
            let cell = match value {
                StoredCollectionPageValue::Scalar(value) => {
                    output.scalar(value, stored_calculation)?
                }
                StoredCollectionPageValue::Collection(cell) => {
                    output.work(columns.len() as u64)?;
                    let collection = page
                        .collection_column(column)
                        .ok_or(InspectError::Malformed)?;
                    let assigned = cell.key().is_some();
                    let mut members = 0_usize;
                    for item in cell.items()? {
                        output.work(1)?;
                        match (collection, item?) {
                            (
                                StoredCollectionColumn::Multivalue(_),
                                StoredCollectionItem::Value(_),
                            )
                            | (
                                StoredCollectionColumn::Attachments(_),
                                StoredCollectionItem::Attachment(_),
                            )
                            | (
                                StoredCollectionColumn::VersionHistory(_),
                                StoredCollectionItem::Version(_),
                            ) => {}
                            _ => return Err(InspectError::Malformed.into()),
                        }
                        members = members.checked_add(1).ok_or(InspectError::StructureLimit)?;
                    }
                    if !assigned && members != 0 {
                        return Err(InspectError::Malformed.into());
                    }
                    let (kind, singular, plural) = match collection {
                        StoredCollectionColumn::Multivalue(_) => (
                            CellKind::Multivalue { assigned, members },
                            "value",
                            "values",
                        ),
                        StoredCollectionColumn::Attachments(_) => (
                            CellKind::Attachments { assigned, members },
                            "attachment",
                            "attachments",
                        ),
                        StoredCollectionColumn::VersionHistory(_) => (
                            CellKind::VersionHistory { assigned, members },
                            "version",
                            "versions",
                        ),
                    };
                    let text = if assigned {
                        let noun = if members == 1 { singular } else { plural };
                        output.text(&format!("{members} {noun}"))?
                    } else {
                        output.text("NULL")?
                    };
                    Cell {
                        text,
                        kind,
                        alignment: HorizontalAlignment::Left,
                    }
                }
            };
            row.push(cell);
        }
        rows.push(row);
    }
    Ok(rows)
}
