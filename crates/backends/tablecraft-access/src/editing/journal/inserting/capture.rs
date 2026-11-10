//! Metadata selects insertion image ownership before any whole-image allocation.
use super::NativeJournalInserter;
#[cfg(feature = "ace16-create")]
use std::io::Cursor;
use std::io::{Read, Seek};
use tablecraft_ace::{InspectError, NativeImage, NativeImageSource};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

impl NativeJournalInserter {
    /// Capture the actual selected INSERT source from the same retained reader.
    /// Metadata selection grants no mutation authority; finish requalifies it.
    pub fn capture_image<R: Read + Seek>(
        source: &mut R,
        scope: CatalogScope,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeImage, InspectError> {
        #[cfg(feature = "ace16-create")]
        if NativeImage::select_retained_table_source(source, scope, table, budget, cancelled)? {
            return NativeImage::capture_retained(source, scope, budget, cancelled);
        }
        #[cfg(not(feature = "ace16-create"))]
        let _ = table;
        NativeImage::capture(source, scope, budget, cancelled)
    }

    /// Inspect shared immutable input under its metadata-selected capture policy.
    /// Sharing retains the whole allocation witness; no second payload is copied.
    pub fn capture_shared_image(
        source: NativeImageSource,
        scope: CatalogScope,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeImage, InspectError> {
        #[cfg(feature = "ace16-create")]
        if NativeImage::select_retained_table_source(
            &mut Cursor::new(source.as_bytes()),
            scope,
            table,
            budget,
            cancelled,
        )? {
            return NativeImage::capture_shared_retained(source, scope, budget, cancelled);
        }
        #[cfg(not(feature = "ace16-create"))]
        let _ = table;
        NativeImage::capture_shared(source, scope, budget, cancelled)
    }
}
