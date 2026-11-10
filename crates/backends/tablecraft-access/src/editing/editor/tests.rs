//! Source refusal disables assignment without trying another writer family.
use super::{JournalRoute, NativeEditor};
use crate::editing::NativeEditError;
use std::io::Cursor;
use tablecraft_ace::{EditError, InspectError, NativeImage, WriteBlocker};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::UpdateProfile;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::opaque_rows as raw;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

#[test]
fn unsupported_opaque_source_keeps_readable_editor_unavailable_without_fallback() {
    let mut source = raw::source(raw::Case::FixedRetired);
    // The LONG descriptor's unknown extended flag remains readable metadata,
    // but it cannot certify absence of required native behavior for any writer.
    let extended = source.definition as usize * 4096 + 100 + 16;
    assert_eq!(source.bytes[extended], 0);
    source.bytes[extended] = 0x80;
    let image = NativeImage::capture_retained(
        &mut Cursor::new(&source.bytes),
        CatalogScope::new(1648),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        image.select_native_retained_opaque_rows_table("Table1", &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::ActiveContent))
    ));
    let editor = NativeEditor::new_journal(image, "Table1", &mut budget(), &|| false).unwrap();
    let (profile, route) = editor.journal_profile(&mut budget(), &|| false).unwrap();
    assert_eq!(profile, UpdateProfile::Unavailable);
    assert!(route == JournalRoute::RetainedOpaqueRows);

    assert!(matches!(
        editor.journal_profile(&mut budget(), &|| true),
        Err(NativeEditError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        editor.journal_profile(&mut ResourceBudget::new(0, 1_000_000, 500_000_000), &|| {
            false
        }),
        Err(NativeEditError::Inspect(InspectError::Limit(_)))
    ));
}
