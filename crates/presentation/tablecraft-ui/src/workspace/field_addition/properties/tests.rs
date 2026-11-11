use super::Properties;
use crate::workspace::field_addition::FieldAdditionChoices;
use std::{num::NonZeroU8, sync::Arc};
use tablecraft_builders::table::{
    DraftNumberSize, FieldAdditionDeclaration, FieldAdditionKind, TypedFieldAdditionAvailability,
    TypedFieldAdditionDeclaration,
};
use tablecraft_schema::ScalarType;

fn choices(declarations: &[TypedFieldAdditionDeclaration]) -> FieldAdditionChoices {
    FieldAdditionChoices {
        number: false,
        short_text: false,
        typed_declarations: Arc::from(
            declarations
                .iter()
                .map(|declaration| TypedFieldAdditionAvailability {
                    declaration: *declaration,
                    maximum_name_utf16_units: 64,
                })
                .collect::<Vec<_>>(),
        ),
    }
}

#[test]
fn each_gallery_family_and_number_size_requires_its_own_observation() {
    let currency = TypedFieldAdditionDeclaration::fixed(ScalarType::Currency).unwrap();
    let decimal = TypedFieldAdditionDeclaration::decimal(18, 0).unwrap();
    let facts = choices(&[currency, decimal]);
    assert!(facts.supports(FieldAdditionKind::Currency));
    assert!(facts.supports(FieldAdditionKind::Number));
    assert!(!facts.supports(FieldAdditionKind::ShortText));
    assert!(!facts.supports(FieldAdditionKind::YesNo));
    assert!(!facts.supports(FieldAdditionKind::DateTime));
    assert!(!facts.supports(FieldAdditionKind::LargeInteger));
    assert!(facts.supports_scalar(ScalarType::Decimal));
    assert!(!facts.supports_scalar(ScalarType::Int32));
    let selected = facts
        .default_declaration(FieldAdditionKind::Number)
        .unwrap();
    assert_eq!(selected, FieldAdditionDeclaration::Typed(decimal));
    let mut properties = Properties::new(selected);
    properties.precision = 28;
    properties.scale = 4;
    assert_eq!(
        properties.declaration(FieldAdditionKind::Number, &facts),
        Some(FieldAdditionDeclaration::Typed(
            TypedFieldAdditionDeclaration::decimal(28, 4).unwrap()
        ))
    );
    properties.decimal_invalid = true;
    assert!(properties.invalid());
    properties.decimal_invalid = false;
    assert!(!properties.invalid());
}

#[test]
fn all_seven_number_sizes_and_native_text_settings_are_validated_declarations() {
    let sizes = [
        DraftNumberSize::Byte,
        DraftNumberSize::Integer,
        DraftNumberSize::LongInteger,
        DraftNumberSize::Single,
        DraftNumberSize::Double,
        DraftNumberSize::Decimal,
        DraftNumberSize::ReplicationId,
    ];
    for size in sizes {
        let declaration = if size == DraftNumberSize::Decimal {
            TypedFieldAdditionDeclaration::decimal(18, 0).unwrap()
        } else {
            TypedFieldAdditionDeclaration::fixed(size.scalar_type()).unwrap()
        };
        let facts = choices(&[declaration]);
        let properties = Properties::new(FieldAdditionDeclaration::Typed(declaration));
        assert_eq!(properties.number, size);
        assert_eq!(
            properties.declaration(FieldAdditionKind::Number, &facts),
            Some(FieldAdditionDeclaration::Typed(declaration))
        );
    }
    let text = TypedFieldAdditionDeclaration::short_text(NonZeroU8::MAX, true);
    let facts = choices(&[text]);
    let mut properties = Properties::new(FieldAdditionDeclaration::Typed(text));
    properties.units = NonZeroU8::new(42).unwrap();
    properties.compression = false;
    assert_eq!(
        properties.declaration(FieldAdditionKind::ShortText, &facts),
        Some(FieldAdditionDeclaration::Typed(
            TypedFieldAdditionDeclaration::short_text(NonZeroU8::new(42).unwrap(), false)
        ))
    );
    properties.size_invalid = true;
    assert!(properties.invalid());
}
