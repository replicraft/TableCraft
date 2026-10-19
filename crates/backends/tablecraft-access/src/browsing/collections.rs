//! Explicit stored attachments and visible-Memo history browsing.
use super::NativeBrowser;
use std::io::{Read, Seek};
use tablecraft_ace::InspectError;
pub use tablecraft_ace::{AttachmentColumn, StoredAttachment, StoredVersion, VersionHistoryColumn};

impl<R: Read + Seek> NativeBrowser<'_, R> {
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
