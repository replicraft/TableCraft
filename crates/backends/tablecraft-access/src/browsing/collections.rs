//! Explicit stored attachments and visible-Memo history browsing.
use super::NativeBrowser;
use tablecraft_ace::InspectError;
pub use tablecraft_ace::{
    AttachmentColumn, CollectionPageRequest, StoredAttachment, StoredCollectionColumn,
    StoredCollectionItem, StoredCollectionPageCell, StoredCollectionPageValue,
    StoredCollectionTablePage, StoredVersion, VersionHistoryColumn,
};

impl<R: tablecraft_ace::NativeReadSource> NativeBrowser<'_, R> {
    /// Open a source-bound typed multivalue reader under the original controls.
    /// Complete parent/child key membership is checked before return. Values are
    /// decoded only for each explicitly requested parent; no complete query cache
    /// is populated. The browser remains exclusively borrowed until reader drop.
    pub fn multivalue_reader(
        &mut self,
        table: &str,
        column: &str,
        limits: tablecraft_ace::TableSnapshotLimits,
    ) -> Result<tablecraft_ace::MultivalueReader<'_, R>, InspectError> {
        tablecraft_ace::MultivalueReader::open(
            &mut self.source,
            self.scope,
            table,
            column,
            limits,
            &mut self.budget,
            self.cancelled,
        )
    }

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
    /// Browse an explicit parent page after complete collection validation under
    /// the browser's unchanged cumulative controls and requested table ceilings.
    pub fn collection_page_with_request(
        &mut self,
        table: &str,
        request: CollectionPageRequest,
    ) -> Result<StoredCollectionTablePage, InspectError> {
        tablecraft_ace::read_stored_collection_table_page_with_request(
            &mut self.source,
            self.scope,
            table,
            request,
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
