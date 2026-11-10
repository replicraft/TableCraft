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
    // Keep MR2 lengths and property carriers intact while replacing a known
    // dictionary key with an equally sized key whose native behavior is unknown.
    // Inert reading preserves it; mutation cannot certify its semantics.
    let known: Vec<u8> = "DefaultValue"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let unknown: Vec<u8> = "UnknownValue"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert_eq!(known.len(), unknown.len());
    let offsets: Vec<usize> = source
        .bytes
        .windows(known.len())
        .enumerate()
        .filter_map(|(at, bytes)| (bytes == known).then_some(at))
        .collect();
    assert!(!offsets.is_empty());
    for at in offsets {
        source.bytes[at..at + unknown.len()].copy_from_slice(&unknown);
    }
    let image = NativeImage::capture_retained(
        &mut Cursor::new(&source.bytes),
        CatalogScope::new(1648),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let selected =
        image.select_native_retained_opaque_rows_table("Table1", &mut budget(), &|| false);
    assert!(
        matches!(
            selected,
            Err(EditError::Unsupported(WriteBlocker::Properties))
        ),
        "{selected:?}"
    );
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
