//! Metadata selects snapshot ownership before allocating the complete edit source.
use super::NativeJournalEditor;
#[cfg(feature = "ace16-create")]
use std::io::Cursor;
use std::io::{Read, Seek};
use tablecraft_ace::{InspectError, NativeImage, NativeImageSource};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

impl NativeJournalEditor {
    /// Capture a metadata-selected source from the caller's retained descriptor.
    /// This selects image ownership only; actual native editing is checked later.
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

    /// Retain one immutable source allocation under its selected native policy.
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
