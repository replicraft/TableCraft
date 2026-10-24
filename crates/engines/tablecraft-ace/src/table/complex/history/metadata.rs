use super::{record, schema::Columns};
use crate::{ComplexColumnLinks, InspectError, NativeColumn, NativeSortOrder};
use tablecraft_core::values::{LegacyDateTime, Value};

/// Complete stored history for one visible Memo and its resolved hidden column.
pub struct VersionHistoryColumn {
    pub(super) links: ComplexColumnLinks,
    pub(super) memo_position: usize,
    pub(super) columns: Columns,
}
impl VersionHistoryColumn {
    pub(in crate::table::complex) fn record<'a>(
        &self,
        row: &'a [Value],
    ) -> Result<StoredVersion<'a>, InspectError> {
        record::read(row, &self.columns)
    }
    /// Checked raw hidden-column links and scoped child identities.
    pub fn links(&self) -> &ComplexColumnLinks {
        &self.links
    }
    /// Visible Memo declaration associated by the conceptual value field name.
    pub fn memo_column(&self) -> &NativeColumn {
        &self.links.schema().owner().columns()[self.memo_position]
    }

    /// Borrow a checked stored value/modified member by its actual native name.
    /// Unknown members and absent ordinals return None; stored NULL remains a value.
    pub fn member_value(&self, parent: usize, child: usize, member: &str) -> Option<&Value> {
        let position = [self.columns.value, self.columns.modified]
            .into_iter()
            .find(|position| {
                self.links.child_snapshot().schema().columns()[*position]
                    .name()
                    .eq_ignore_ascii_case(member)
            })?;
        self.links.child_row(parent, child)?.get(position)
    }

    /// Exact flat Text order for a declared stored member, without I/O.
    pub fn member_text_sort_order(&self, member: &str) -> Option<NativeSortOrder> {
        [self.columns.value, self.columns.modified]
            .into_iter()
            .map(|position| &self.links.child_snapshot().schema().columns()[position])
            .find(|column| column.name().eq_ignore_ascii_case(member))?
            .text_sort_order()
    }

    /// Stored versions in flat snapshot order, without newest-first sorting or
    /// implicit latest-value selection. An absent parent ordinal returns None.
    pub fn stored_versions(
        &self,
        parent: usize,
    ) -> Option<impl ExactSizeIterator<Item = Result<StoredVersion<'_>, InspectError>>> {
        Some(
            self.links
                .child_rows(parent)?
                .map(|row| record::read(row, &self.columns)),
        )
    }
}

/// Borrowed stored version; neither text nor timestamp is synthesized.
pub struct StoredVersion<'a> {
    pub(super) identity: i32,
    pub(super) value: Option<&'a str>,
    pub(super) modified: Option<LegacyDateTime>,
}
impl<'a> StoredVersion<'a> {
    /// Positive child identity scoped to the hidden flat table snapshot.
    pub const fn identity(&self) -> i32 {
        self.identity
    }
    /// Exact stored Memo text; NULL and empty text remain distinct.
    pub const fn value(&self) -> Option<&'a str> {
        self.value
    }
    /// Exact stored legacy DATE serial, without timezone/calendar rounding.
    pub const fn modified(&self) -> Option<LegacyDateTime> {
        self.modified
    }
}
