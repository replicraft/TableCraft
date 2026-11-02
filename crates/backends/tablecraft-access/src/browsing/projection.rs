//! Explicit buffered scalar selection beside unselected mixed-schema payloads.
use super::NativeBrowser;
use tablecraft_ace::{InspectError, NativeReadSource, TablePage, TablePageRequest};

impl<R: NativeReadSource> NativeBrowser<'_, R> {
    /// Read a bounded ordinary scalar subset under the browser's original controls.
    /// Full native schema remains visible; unselected values are not decoded or
    /// certified. Selected cached/collection fields retain their ordinary refusal.
    /// Defaults, complete collection reads and cursor continuation are unchanged.
    pub fn projected_page(
        &mut self,
        table: &str,
        names: &[&str],
        request: TablePageRequest,
    ) -> Result<TablePage, InspectError> {
        tablecraft_ace::read_table_projection_page(
            &mut self.source,
            self.scope,
            table,
            names,
            request,
            &mut self.budget,
            self.cancelled,
        )
    }
}
