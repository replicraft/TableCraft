//! Explicit stored attachments and visible-Memo history browsing.
use super::NativeBrowser;
use tablecraft_ace::InspectError;
pub use tablecraft_ace::{
    AttachmentColumn, StoredAttachment, StoredCollectionColumn, StoredCollectionItem,
    StoredCollectionPageCell, StoredCollectionPageValue, StoredCollectionTablePage, StoredVersion,
    VersionHistoryColumn,
};

impl<R: tablecraft_ace::NativeReadSource> NativeBrowser<'_, R> {
    /// Browse a parent page with all qualified stored collection types.
    /// Full parent/child validation retains existing snapshot limits. Encoded
    /// attachments stay inert, history stays in stored order and formulas never run.
    pub fn collection_page(
        &mut self,
        table: &str,
        request: tablecraft_ace::TablePageRequest,
        include_stored_calculations: bool,
    ) -> Result<StoredCollectionTablePage, InspectError> {
        tablecraft_ace::read_stored_collection_table_page(
            &mut self.source,
            self.scope,
            table,
            request,
            include_stored_calculations,
            &mut self.budget,
            self.cancelled,
        )
    }
    /// Read complete checked stored attachment metadata and encoded payloads.
    /// No payload decoding, URL resolution, file extraction or execution occurs.
    pub fn attachment_column(
        &mut self,
        table: &str,
        column: &str,
    ) -> Result<AttachmentColumn, InspectError> {
        tablecraft_ace::read_attachment_column(
            &mut self.source,
            self.scope,
            table,
            column,
            &mut self.budget,
            self.cancelled,
        )
    }
    /// Read complete checked stored history associated with a visible Memo field.
    /// Retains flat snapshot order and exact timestamps; no latest-value selection.
    pub fn version_history_column(
        &mut self,
        table: &str,
        memo: &str,
    ) -> Result<VersionHistoryColumn, InspectError> {
        tablecraft_ace::read_version_history_column(
            &mut self.source,
            self.scope,
            table,
            memo,
            &mut self.budget,
            self.cancelled,
        )
    }
}
