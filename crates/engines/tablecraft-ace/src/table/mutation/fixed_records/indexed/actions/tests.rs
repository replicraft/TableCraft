//! Read-fact family does not grant UPDATE targets or accept foreign image seals.
use super::super::CheckedIndexes;
use crate::{EditError, NativeImage, WriteBlocker};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_integral_indexed as sources;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires 34 native/SDK-qualified numeric source views"]
fn action_capture_is_source_bound_and_preserves_i32_update_admission() {
    let cases = sources::load_all(|raw, expected| {
        let hash = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(hash, expected);
    });
    for case in cases
        .into_iter()
        .filter(|case| case.operation == "insert" && case.recipe == "new-family")
    {
        let scope = CatalogScope::new(199);
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            scope,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let old = CheckedIndexes::capture(&image, case.table, &mut budget(), &|| false)
            .err()
            .unwrap();
        assert_eq!(old, EditError::Unsupported(WriteBlocker::IndexedColumn));
        let action =
            CheckedIndexes::capture_actions(&image, case.table, &mut budget(), &|| false).unwrap();
        assert_eq!(action.checked().indexes().len(), 6);
        action
            .require_source(&image, action.checked().schema(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            action.require_target(action.checked().schema(), 1, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
        let foreign = NativeImage::capture(
            &mut Cursor::new(&case.before),
            scope,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            action.require_source(&foreign, action.checked().schema(), &mut budget(), &|| {
                false
            }),
            Err(EditError::Preservation)
        );
        assert_eq!(image.as_bytes(), case.before);
    }
}
