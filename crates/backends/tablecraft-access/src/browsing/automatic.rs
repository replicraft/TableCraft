//! Metadata-selected inert browsing without retrying failed record reads.
use super::{CollectionPageRequest, NativeBrowser, StoredCollectionTablePage, TablePage};
use tablecraft_ace::{InspectError, NativeReadSource};

/// A buffered native page retaining ordinary scalar or explicit stored collection cells.
/// Cached calculations are stored results only; attachment bytes remain encoded.
pub enum BrowsePage {
    /// Ordinary scalar paging under the existing visible-page limits.
    Scalar(TablePage),
    /// Qualified collections and cached calculations under complete collection limits.
    Collections(StoredCollectionTablePage),
}

impl<R: NativeReadSource> NativeBrowser<'_, R> {
    /// Select the existing reader from metadata, then read exactly that profile.
    /// Ordinary schemas retain bounded page reads without a complete-table snapshot.
    /// Other schemas must qualify every column through stored collection browsing,
    /// including cached scalar results; formulas, attachment decoders and providers
    /// never execute. Unsupported columns fail rather than being omitted or retried.
    pub fn browse_page(
        &mut self,
        table: &str,
        request: CollectionPageRequest,
    ) -> Result<BrowsePage, InspectError> {
        let description = self.describe(table)?;
        if description.scalar_columns_supported() {
            self.page(table, request.page()).map(BrowsePage::Scalar)
        } else {
            self.collection_page_with_request(table, request.with_stored_calculations(true))
                .map(BrowsePage::Collections)
        }
    }
}
