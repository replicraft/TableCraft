use super::super::{attachments, history, multivalue};
use crate::{
    AttachmentColumn, ComplexColumnLinks, InspectError, MultivalueColumn, StoredAttachment,
    StoredVersion, VersionHistoryColumn,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};

/// A qualified stored collection declaration; no SQL scalar/flattening semantics.
pub enum StoredCollectionColumn {
    /// Stored scalar multivalue list.
    Multivalue(MultivalueColumn),
    /// Stored attachment metadata and encoded file bytes.
    Attachments(AttachmentColumn),
    /// Stored versions associated with a visible Memo.
    VersionHistory(VersionHistoryColumn),
}
impl StoredCollectionColumn {
    /// Checked parent membership and child snapshot shared by every variant.
    pub fn links(&self) -> &ComplexColumnLinks {
        match self {
            Self::Multivalue(column) => column.links(),
            Self::Attachments(column) => column.links(),
            Self::VersionHistory(column) => column.links(),
        }
    }
    pub(super) fn record<'a>(
        &self,
        row: &'a [Value],
    ) -> Result<StoredCollectionItem<'a>, InspectError> {
        Ok(match self {
            Self::Multivalue(column) => StoredCollectionItem::Value(column.record(row)?),
            Self::Attachments(column) => StoredCollectionItem::Attachment(column.record(row)?),
            Self::VersionHistory(column) => StoredCollectionItem::Version(column.record(row)?),
        })
    }
}

/// One explicitly typed stored child item; encoded payloads remain inert.
pub enum StoredCollectionItem<'a> {
    /// An exact stored scalar list element, including NULL.
    Value(&'a Value),
    /// Borrowed attachment metadata and encoded bytes.
    Attachment(StoredAttachment<'a>),
    /// Borrowed Memo version and exact timestamp.
    Version(StoredVersion<'a>),
}

pub(super) fn read_links(
    links: ComplexColumnLinks,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<StoredCollectionColumn, InspectError> {
    // Field count routes to a strict existing reader, and never qualifies a type
    // by itself. Unknown/malformed shapes do not fall back to a different decoder.
    match links.column().type_table().columns() {
        [_] => Ok(StoredCollectionColumn::Multivalue(multivalue::read_links(
            links, budget, cancelled,
        )?)),
        [_, _] => {
            let value = links
                .column()
                .type_table()
                .columns()
                .iter()
                .find(|field| field.native_type() == crate::format::data_type::LONG_TEXT)
                .ok_or(InspectError::Unsupported)?;
            let (position, memo) =
                history::select(links.schema(), value.name(), budget, cancelled)?;
            if links.schema().owner().columns()[position].number() != links.column().number() {
                return Err(InspectError::Malformed);
            }
            Ok(StoredCollectionColumn::VersionHistory(history::read_links(
                links, memo, budget, cancelled,
            )?))
        }
        [_, _, _, _, _, _] => Ok(StoredCollectionColumn::Attachments(
            attachments::read_links(links, budget, cancelled)?,
        )),
        _ => Err(InspectError::Unsupported),
    }
}
