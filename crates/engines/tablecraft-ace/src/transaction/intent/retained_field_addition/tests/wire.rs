//! Exact TCAC12 framing, gallery defaults and closed older declaration grammars.
use super::literal;
use crate::{
    CurrentFieldAdditionIntentError, InspectError, NativeCurrentFieldAdditionIntent,
    NativeRetainedTypedFieldAdditionIntent as Intent,
    RetainedTypedFieldAdditionIntentError as Error,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields, typed_fields as raw,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

#[test]
fn typed_field_addition_originals_match_twelve_independent_wire_and_default_goldens() {
    let source = fields::source(fields::Case::FirstNumber);
    for kind in raw::KINDS {
        let good = literal(&source.bytes, kind);
        let intent = Intent::decode(&good, &mut budget(), &|| false).unwrap();
        assert_eq!(intent.table(), "Table1");
        assert_eq!(intent.column(), "AddedValue");
        intent
            .verify_before(&source.bytes, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), good);
        let reconstructed = Intent::new(
            "Table1",
            crate::RetainedFieldAdditionDefinition::capture(
                intent.definition().column(),
                &mut budget(),
                &|| false,
            )
            .unwrap(),
            &source.bytes,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            reconstructed.encode(&mut budget(), &|| false).unwrap(),
            good
        );
        for decode in [
            NativeCurrentFieldAdditionIntent::decode,
            NativeCurrentFieldAdditionIntent::decode_short_text,
            NativeCurrentFieldAdditionIntent::decode_retained,
        ] {
            assert!(matches!(
                decode(&good, &mut budget(), &|| false),
                Err(CurrentFieldAdditionIntentError::InvalidRecord)
            ));
        }
        let mut changed = source.bytes.clone();
        let last = changed.len() - 1;
        changed[last] ^= 1;
        assert!(matches!(
            intent.verify_before(&changed, &mut budget(), &|| false),
            Err(Error::BeforeMismatch)
        ));
    }
}

#[test]
fn typed_field_addition_framing_and_non_gallery_defaults_refuse_before_publication() {
    let source = fields::source(fields::Case::FirstNumber);
    let good = literal(&source.bytes, raw::Kind::Currency);
    for at in [0, 8, 12, 13, 61, 67, 70, 71] {
        let mut invalid = good.clone();
        invalid[at] ^= 1;
        assert!(matches!(
            Intent::decode(
                &invalid,
                &mut ResourceBudget::new(0, 1000, 1_000_000),
                &|| false
            ),
            Err(Error::InvalidRecord)
        ));
    }
    for at in [60, 62, 64, 65, 66] {
        let mut invalid = good.clone();
        invalid[at] = if at == 60 { 9 } else { 1 };
        assert!(matches!(
            Intent::decode(&invalid, &mut budget(), &|| false),
            Err(Error::InvalidRecord)
        ));
    }
    for kind in raw::KINDS {
        let mut invalid = literal(&source.bytes, kind);
        if invalid.len() > 88 {
            invalid[88] = 1;
            assert!(Intent::decode(&invalid, &mut budget(), &|| false).is_err());
        }
    }
    let mut old = good.clone();
    old[..8].copy_from_slice(b"TCAC0011");
    assert!(matches!(
        Intent::decode(&old, &mut budget(), &|| false),
        Err(Error::InvalidRecord)
    ));
    assert!(matches!(
        Intent::decode(&good, &mut budget(), &|| true),
        Err(Error::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        Intent::decode(&good, &mut ResourceBudget::new(0, 1000, 1_000_000), &|| {
            false
        }),
        Err(
            Error::Declaration(crate::CreateDatabaseError::Inspect(InspectError::Limit(_)))
                | Error::Inspect(InspectError::Limit(_))
        )
    ));
}
