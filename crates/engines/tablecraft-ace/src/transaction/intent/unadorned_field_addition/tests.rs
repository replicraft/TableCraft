//! Independent literal TCAC14 frames keep every declaration parameter explicit.
mod recovery;

use super::{
    NativeUnadornedFieldAdditionIntent as Intent, UnadornedFieldAdditionIntentError as Error,
};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

#[derive(Clone, Copy)]
struct Shape {
    tag: u8,
    width: u16,
    precision: u8,
    scale: u8,
}

const SHAPES: [Shape; 12] = [
    Shape {
        tag: 1,
        width: 0,
        precision: 0,
        scale: 0,
    },
    Shape {
        tag: 2,
        width: 1,
        precision: 0,
        scale: 0,
    },
    Shape {
        tag: 3,
        width: 2,
        precision: 0,
        scale: 0,
    },
    Shape {
        tag: 4,
        width: 4,
        precision: 0,
        scale: 0,
    },
    Shape {
        tag: 19,
        width: 8,
        precision: 0,
        scale: 0,
    },
    Shape {
        tag: 6,
        width: 4,
        precision: 0,
        scale: 0,
    },
    Shape {
        tag: 7,
        width: 8,
        precision: 0,
        scale: 0,
    },
    Shape {
        tag: 5,
        width: 8,
        precision: 0,
        scale: 0,
    },
    Shape {
        tag: 8,
        width: 8,
        precision: 0,
        scale: 0,
    },
    Shape {
        tag: 15,
        width: 16,
        precision: 0,
        scale: 0,
    },
    Shape {
        tag: 10,
        width: 160,
        precision: 0,
        scale: 0,
    },
    Shape {
        tag: 16,
        width: 17,
        precision: 28,
        scale: 4,
    },
];

fn literal(before: &[u8], table: &str, definition: u32, shape: Shape, source: u8) -> Vec<u8> {
    literal_named(before, table, "AddedValue", definition, shape, source)
}

fn literal_named(
    before: &[u8],
    table: &str,
    column: &str,
    definition: u32,
    shape: Shape,
    source: u8,
) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut wire = b"TCAC0014".to_vec();
    wire.extend(((88 + table.len() + column.len()) as u32).to_le_bytes());
    wire.extend(14_u16.to_le_bytes());
    wire.extend(0_u16.to_le_bytes());
    wire.extend((before.len() as u64).to_le_bytes());
    wire.extend(digest.finalize());
    wire.extend((definition as i32).to_le_bytes());
    wire.extend(definition.to_le_bytes());
    wire.extend(2_u16.to_le_bytes());
    wire.extend(2_u16.to_le_bytes());
    wire.extend((if source == 1 { 2_u16 } else { 3_u16 }).to_le_bytes());
    wire.extend([shape.tag, 0]);
    wire.extend((table.len() as u16).to_le_bytes());
    wire.extend((column.len() as u16).to_le_bytes());
    wire.extend([source, 0]);
    wire.extend(shape.width.to_le_bytes());
    wire.extend([shape.precision, shape.scale]);
    wire.extend([0; 6]);
    wire.extend(table.as_bytes());
    wire.extend(column.as_bytes());
    wire
}

#[test]
fn literal_twelve_scalar_carriers_and_source_witnesses_roundtrip_without_gallery_policies() {
    let before = vec![0; 20 * 4096];
    let kinds = [
        ScalarType::Boolean,
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int32,
        ScalarType::Int64,
        ScalarType::Single,
        ScalarType::Double,
        ScalarType::Currency,
        ScalarType::DateTime,
        ScalarType::Guid,
        ScalarType::Text,
        ScalarType::Decimal,
    ];
    for (shape, kind) in SHAPES.into_iter().zip(kinds) {
        for source in [1, 2] {
            let wire = literal(&before, "項目", 14, shape, source);
            let intent = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
            let column = intent.definition().column();
            assert_eq!(column.scalar_type(), kind);
            assert_eq!(column.required(), None);
            assert_eq!(column.allow_zero_length(), None);
            assert!(column.default_literal().is_none());
            assert!(column.description().is_none());
            assert!(!column.unicode_compression());
            if kind == ScalarType::Text {
                assert_eq!(column.max_text_utf16_units(), Some(80));
            }
            if kind == ScalarType::Decimal {
                let constraints = column.decimal_constraints().unwrap();
                assert_eq!((constraints.precision(), constraints.scale()), (28, 4));
            }
            assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
            intent
                .verify_before(&before, &mut budget(), &|| false)
                .unwrap();
            assert!(
                crate::NativeDeclaredFieldAdditionIntent::decode(&wire, &mut budget(), &|| false)
                    .is_err()
            );
            assert!(
                crate::NativeRetainedTypedFieldAdditionIntent::decode(
                    &wire,
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
        }
    }
}

#[test]
fn original14_refuses_noncanonical_width_parameters_flags_names_and_incomplete_frames() {
    let before = vec![0; 20 * 4096];
    for shape in SHAPES {
        let wire = literal(&before, "Items", 14, shape, 1);
        for end in 0..wire.len() {
            assert!(Intent::decode(&wire[..end], &mut budget(), &|| false).is_err());
        }
        for (offset, value) in [
            (0, b'X'),
            (7, b'3'),
            (12, 13),
            (14, 1),
            (71, 1),
            (76, 0),
            (76, 3),
            (77, 1),
            (82, 1),
            (87, 1),
        ] {
            let mut invalid = wire.to_vec();
            invalid[offset] = value;
            assert!(
                Intent::decode(&invalid, &mut budget(), &|| false).is_err(),
                "offset {offset}"
            );
        }
        let mut invalid = wire.to_vec();
        invalid[78] ^= 1;
        assert!(Intent::decode(&invalid, &mut budget(), &|| false).is_err());
        let mut trailing = wire.to_vec();
        trailing.push(0);
        assert!(Intent::decode(&trailing, &mut budget(), &|| false).is_err());
    }
    for (tag, width, precision, scale) in [
        (9, 16, 0, 0),
        (20, 42, 0, 0),
        (10, 0, 0, 0),
        (10, 512, 0, 0),
        (10, 159, 0, 0),
        (16, 17, 0, 0),
        (16, 17, 29, 0),
        (16, 17, 4, 5),
        (4, 4, 1, 0),
    ] {
        let wire = literal(
            &before,
            "Items",
            14,
            Shape {
                tag,
                width,
                precision,
                scale,
            },
            1,
        );
        assert!(Intent::decode(&wire, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn original14_binding_allocation_work_and_cancellation_remain_typed() {
    let before = vec![0; 20 * 4096];
    let wire = literal(&before, "Items", 14, SHAPES[7], 1);
    assert!(matches!(
        Intent::decode(&wire, &mut budget(), &|| true),
        Err(Error::Inspect(crate::InspectError::Cancelled))
    ));
    assert!(matches!(
        Intent::decode(
            &wire,
            &mut ResourceBudget::new(0, 1_000_000, 500_000_000),
            &|| false
        ),
        Err(Error::Declaration(crate::CreateDatabaseError::Inspect(
            crate::InspectError::Limit(tablecraft_core::limits::LimitError {
                resource: tablecraft_core::limits::Resource::Bytes,
                ..
            })
        )))
    ));
    let intent = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
    let mut changed = before.to_vec();
    changed[7000] ^= 1;
    assert!(matches!(
        intent.verify_before(&changed, &mut budget(), &|| false),
        Err(Error::BeforeMismatch)
    ));
}

#[test]
fn original14_native_names_and_parameter_boundaries_remain_explicit() {
    let before = vec![0; 20 * 4096];
    for table in ["界".repeat(64), "🦀".repeat(32)] {
        let wire = literal_named(&before, &table, &"A".repeat(64), 14, SHAPES[7], 1);
        assert_eq!(
            Intent::decode(&wire, &mut budget(), &|| false)
                .unwrap()
                .encode(&mut budget(), &|| false)
                .unwrap(),
            wire
        );
    }
    for (table, column) in [
        ("界".repeat(65), "Amount".into()),
        ("Items".into(), "A".repeat(65)),
        ("Items".into(), "ſ".into()),
        ("Items".into(), "bad.name".into()),
    ] {
        assert!(
            Intent::decode(
                &literal_named(&before, &table, &column, 14, SHAPES[7], 1),
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    for shape in [
        Shape {
            tag: 10,
            width: 2,
            precision: 0,
            scale: 0,
        },
        Shape {
            tag: 10,
            width: 510,
            precision: 0,
            scale: 0,
        },
        Shape {
            tag: 16,
            width: 17,
            precision: 1,
            scale: 0,
        },
        Shape {
            tag: 16,
            width: 17,
            precision: 28,
            scale: 28,
        },
    ] {
        let wire = literal(&before, "Items", 14, shape, 1);
        assert_eq!(
            Intent::decode(&wire, &mut budget(), &|| false)
                .unwrap()
                .encode(&mut budget(), &|| false)
                .unwrap(),
            wire
        );
    }
    let mut signed = literal(&before, "Items", 14, SHAPES[7], 1);
    signed[56..60].copy_from_slice(&(-37_i32).to_le_bytes());
    assert_eq!(
        Intent::decode(&signed, &mut budget(), &|| false)
            .unwrap()
            .native_table_id(),
        -37
    );
}
