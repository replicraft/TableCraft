//! Metadata spans distinguish inline bodies from actual external references.
use super::*;
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fields::{
    self as raw, Case, Target,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(4101),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn name_bytes(name: &str) -> Vec<u8> {
    name.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

#[test]
fn retained_observation_uses_actual_inline_and_single_body_name_spans() {
    for (case, field) in [(Case::FirstNumber, "Score"), (Case::SecondText, "Name")] {
        let source = raw::source(case);
        let original = image(&source.bytes);
        let inline = Observed::capture_retained_schema_rename(
            &original,
            "Table1",
            field,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(inline.identity.definition, source.definition);
        assert_eq!(inline.property_reference, None);
        assert_eq!(
            &source.bytes[inline.property_name.start..inline.property_name.end],
            name_bytes(field),
        );
        assert_eq!(
            &source.bytes[inline.property_body.start..inline.property_body.start + 4],
            b"MR2\0",
        );
        assert!(
            Observed::capture_current_rename(&original, "Table1", field, &mut budget(), &|| false,)
                .is_err()
        );

        for target in [Target::Number, Target::ShortText] {
            let bytes = raw::after(
                &source.bytes,
                source.definition,
                source.catalog_root,
                target,
            );
            let updated = image(&bytes);
            let single = Observed::capture_retained_schema_drop(
                &updated,
                "Table1",
                target.name(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(single.identity.definition, source.definition);
            assert_eq!(single.property_reference.map(|value| value >> 8), Some(10));
            assert_eq!(
                &bytes[single.property_name.start..single.property_name.end],
                name_bytes(target.name()),
            );
            assert_eq!(
                &bytes[single.property_body.start..single.property_body.start + 4],
                b"MR2\0",
            );
        }
    }
}
