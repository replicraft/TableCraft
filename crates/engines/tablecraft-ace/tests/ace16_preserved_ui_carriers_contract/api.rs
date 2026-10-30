//! Actual fixed consumers of source-qualified passive UI representations.
use super::{SCOPE, budget, name_map, source};
use std::io::Cursor;
use tablecraft_ace::{EditError, FixedColumnUpdates, FixedScalarUpdate, NativeImage, WriteBlocker};
use tablecraft_core::values::Value;

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn passive_ui_signed_zero_and_positive_carriers_preserve_complete_one_bit_images() {
    for case in [
        name_map::source(false, false),
        source::source(source::Carrier::WideSigned),
        source::source(source::Carrier::WidePositive),
    ] {
        let updates = [FixedScalarUpdate::new(
            0,
            Value::Boolean(false),
            Value::Boolean(true),
        )];
        let single = capture(&case.before)
            .preview_ace16_fixed_updates("Items", "Yes_Field", &updates, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(single.candidate_bytes(), case.expected);
        assert_eq!(single.changed_rows(), 1);

        let groups = [FixedColumnUpdates::new("Items", "Yes_Field", &updates)];
        let batch = capture(&case.before)
            .preview_ace16_batch_updates(&groups, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(batch.candidate_bytes(), case.expected);
        assert_eq!(batch.changed_rows(), 1);

        let no_op = capture(&case.before)
            .preview_ace16_fixed_updates("Items", "Yes_Field", &[], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(no_op.candidate_bytes(), case.before);
        assert_eq!(no_op.changed_rows(), 0);
    }
}

#[test]
fn passive_ui_unqualified_upper_words_widths_kinds_and_flags_refuse_even_empty_updates() {
    for carrier in [
        source::Carrier::UnqualifiedUpperWord,
        source::Carrier::ThreeBytes,
        source::Carrier::WrongNativeKind,
        source::Carrier::DdlFlag,
    ] {
        let case = source::source(carrier);
        for updates in [
            Vec::new(),
            vec![FixedScalarUpdate::new(
                0,
                Value::Boolean(false),
                Value::Boolean(true),
            )],
        ] {
            assert_eq!(
                capture(&case.before)
                    .preview_ace16_fixed_updates(
                        "Items",
                        "Yes_Field",
                        &updates,
                        &mut budget(),
                        &|| false
                    )
                    .err(),
                Some(EditError::Unsupported(WriteBlocker::Properties))
            );
        }
    }
}
