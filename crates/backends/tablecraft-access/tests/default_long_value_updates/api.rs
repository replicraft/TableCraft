//! One metadata-selected service preserves the explicit LVAL profile.
use super::{cases, forwarding, original};
use std::io::Cursor;
use tablecraft_access::engineering::{AccessEngineeringEditor, ScalarUpdateMode};
use tablecraft_ace::NativeImage;
use tablecraft_core::values::Value;
use tablecraft_db::{CellUpdate, EditableTable};
use tablecraft_model::CatalogScope;

#[test]
fn default_and_explicit_lval_match_all_frozen_images() {
    for case in qualified_cases() {
        for mode in [ScalarUpdateMode::Default, ScalarUpdateMode::LongValue] {
            let mut work = original::budget();
            let image = NativeImage::capture(
                &mut Cursor::new(&case.before),
                CatalogScope::new(1),
                &mut work,
                &|| false,
            )
            .unwrap();
            let editor =
                AccessEngineeringEditor::new(image, "Items", mode, &mut work, &|| false).unwrap();
            let row = editor.snapshot().row_handle(0).unwrap();
            let candidate = editor
                .plan_updates(
                    case.column,
                    &[CellUpdate::new(row, case.value.clone())],
                    &mut work,
                    &|| false,
                )
                .unwrap();
            assert_image(candidate.before_bytes(), &case.before);
            assert_image(candidate.candidate_bytes(), &case.expected);
        }
    }
}

#[test]
fn empty_default_batches_still_choose_native_lval_storage() {
    for case in qualified_cases() {
        let mut work = original::budget();
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            CatalogScope::new(1),
            &mut work,
            &|| false,
        )
        .unwrap();
        let editor = AccessEngineeringEditor::new(
            image,
            "Items",
            ScalarUpdateMode::Default,
            &mut work,
            &|| false,
        )
        .unwrap();
        let candidate = editor
            .plan_updates(case.column, &[], &mut work, &|| false)
            .unwrap();
        assert_image(candidate.candidate_bytes(), &case.before);
    }
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    let first = actual.iter().zip(expected).position(|(a, b)| a != b);
    let count = actual.iter().zip(expected).filter(|(a, b)| a != b).count();
    assert!(
        first.is_none(),
        "complete image mismatch: {count} bytes; first offset {first:?}"
    );
}

// The predecessor compacting expectation remains in its original source owner.
// Only the independently qualified stable-root case is used for API replay.
fn qualified_cases() -> Vec<cases::Case> {
    let mut selected = cases::cases();
    let position = selected
        .iter()
        .position(|case| {
            case.column == "Ole"
                && case.before_value == Value::Binary(vec![7])
                && case.value == Value::Binary(vec![8])
        })
        .expect("the original external OLE replacement case");
    selected[position] = forwarding::case();
    selected
}
