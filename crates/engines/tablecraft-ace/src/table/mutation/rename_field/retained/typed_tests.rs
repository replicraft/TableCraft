//! One typed rename language preserves actual scalar/default/storage metadata.
use crate::{FieldRenameError, InspectError, NativeImage, inspect_table_properties, read_table};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{
        boolean, currency, exact_values, field_changes, fields, fixed_scalars,
    },
    raw_forwarding_chain,
};

const SCOPE: CatalogScope = CatalogScope::new(4207);

#[derive(Clone, Copy, Debug)]
enum Case {
    Fixed,
    Exact,
    Currency,
    Boolean,
    Text,
    Keyless,
}

const CASES: [Case; 6] = [
    Case::Fixed,
    Case::Exact,
    Case::Currency,
    Case::Boolean,
    Case::Text,
    Case::Keyless,
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn source(case: Case) -> (Vec<u8>, u32, usize, &'static [&'static str]) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = u32::try_from(seed.len() / 4096).unwrap();
    let root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let (bytes, names): (Vec<u8>, &'static [&'static str]) = match case {
        Case::Fixed => {
            let empty = fixed_scalars::append_table(&seed, "Values");
            let bytes = fixed_scalars::append_values(
                &empty,
                definition,
                &[
                    fixed_scalars::Scalar::Byte(17),
                    fixed_scalars::Scalar::Int16(-1234),
                    fixed_scalars::Scalar::Int64(9_007_199_254_740_993),
                    fixed_scalars::Scalar::SingleBits(0x3fa0_0000),
                    fixed_scalars::Scalar::DoubleBits(0x3ff3_c0ca_428c_59dd),
                    fixed_scalars::Scalar::DateTimeBits(0x4004_0000_0000_0000),
                ],
            );
            (
                bytes,
                &[
                    "Score",
                    "ByteValue",
                    "ShortValue",
                    "LargeValue",
                    "SingleValue",
                    "DoubleValue",
                    "DateValue",
                ],
            )
        }
        Case::Exact => {
            let empty = exact_values::append_table(&seed, "Values");
            (
                exact_values::append(
                    &empty,
                    definition,
                    Some([0x31; 16]),
                    Some(90_071_992_547_409_931_234),
                ),
                &["Token", "Amount"],
            )
        }
        Case::Currency => {
            let empty = currency::append_table(&seed, "Values");
            (
                currency::append(&empty, definition, currency::Input::Value(12345)),
                &["Amount"],
            )
        }
        Case::Boolean => {
            let empty = boolean::append_table(&seed, "Values");
            (
                boolean::append(&empty, definition, boolean::Input::Value(true)),
                &["Enabled"],
            )
        }
        Case::Text => {
            let empty = currency::mixed::append_table(&seed, "Values");
            let inserted = currency::mixed::append(&empty, definition);
            (
                currency::mixed::update_text(&inserted, definition, 0, Some("original")),
                &["Label"],
            )
        }
        Case::Keyless => (field_changes::keyless_first_field(&seed, "Values"), &["Id"]),
    };
    (bytes, definition, root, names)
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first difference {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

#[test]
fn retained_typed_renames_preserve_all_scalar_kinds_property_values_and_rows() {
    for case in CASES {
        for forwarded in [false, true] {
            let (mut before, definition, root, names) = source(case);
            if forwarded {
                before = raw_forwarding_chain::extend(&before, definition + 1, 0, 20);
                before = raw_forwarding_chain::extend(&before, 4, root, 20);
            }
            let old_rows = read_table(
                &mut Cursor::new(&before),
                SCOPE,
                "Values",
                &mut budget(),
                &|| false,
            )
            .unwrap()
            .into_rows();
            for old in names {
                let lower = old.to_ascii_lowercase();
                for new in ["AConsiderablyLongerName", "X", lower.as_str(), old] {
                    let native = image(&before);
                    let facts = native
                        .inspect_retained_typed_field_rename_source(
                            "Values",
                            old,
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                    let expected = field_changes::rename(&before, definition, root, old, new);
                    let candidate = native
                        .prepare_retained_typed_field_rename(
                            "Values",
                            old,
                            new,
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap_or_else(|error| {
                            panic!("{case:?}/{forwarded}/{old}/{new}: {error:?}")
                        });
                    assert!(candidate.is_retained_typed());
                    assert!(!candidate.is_retained_schema());
                    assert_eq!(candidate.identity(), facts.identity());
                    assert_eq!(candidate.changed(), old != &new);
                    equal_image(candidate.after_bytes(), &expected);
                    assert_eq!(
                        read_table(
                            &mut Cursor::new(candidate.after_bytes()),
                            SCOPE,
                            "Values",
                            &mut budget(),
                            &|| false
                        )
                        .unwrap()
                        .rows(),
                        old_rows
                    );
                    for page in [definition + 1]
                        .into_iter()
                        .chain((!matches!(case, Case::Keyless)).then_some(definition + 2))
                    {
                        let start = page as usize * 4096;
                        assert_eq!(
                            &candidate.after_bytes()[start..start + 4096],
                            &before[start..start + 4096]
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn retained_typed_rename_selection_keeps_legacy_shapes_and_keyless_field_zero_distinct() {
    for case in CASES {
        let (before, definition, root, names) = source(case);
        for name in names {
            let native = image(&before);
            let typed = native
                .select_native_retained_typed_field_rename_source(
                    "Values",
                    name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let original_shape = matches!(*name, "Score" | "Label");
            assert_eq!(typed, !original_shape, "{case:?}/{name}");
            let original = image(&before).prepare_retained_schema_field_rename(
                "Values",
                name,
                "Renamed",
                &mut budget(),
                &|| false,
            );
            if original_shape {
                let original = original.unwrap();
                assert!(original.is_retained_schema());
                assert!(!original.is_retained_typed());
            } else {
                assert!(original.is_err(), "old6 must not admit {case:?}/{name}");
            }
        }
        if matches!(case, Case::Keyless) {
            let candidate = image(&before)
                .prepare_retained_typed_field_rename(
                    "Values",
                    "Id",
                    "Identifier",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(candidate.identity().column_number(), 0);
            equal_image(
                candidate.after_bytes(),
                &field_changes::rename(&before, definition, root, "Id", "Identifier"),
            );
        }
    }
    let (before, _, _, _) = source(Case::Fixed);
    assert!(matches!(
        image(&before).prepare_retained_typed_field_rename(
            "Values",
            "ByteValue",
            "sCoRe",
            &mut budget(),
            &|| false
        ),
        Err(FieldRenameError::DuplicateName)
    ));
    assert!(
        image(&before)
            .prepare_retained_typed_field_rename(
                "Values",
                "ID",
                "Generated",
                &mut budget(),
                &|| false
            )
            .is_err()
    );
}

#[test]
fn retained_typed_rename_still_refuses_unknown_properties_nonliteral_defaults_and_limits() {
    let (before, definition, _, _) = source(Case::Fixed);
    let properties = inspect_table_properties(
        &mut Cursor::new(&before),
        SCOPE,
        "Values",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let body = properties.raw_bytes();
    let start = before
        .windows(body.len())
        .position(|bytes| bytes == body)
        .unwrap();
    for (old, replacement) in [("DefaultValue", "RecordSource"), ("0e0", "Now")] {
        let encoded = old
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let position = body
            .windows(encoded.len())
            .position(|bytes| bytes == encoded)
            .unwrap();
        let changed = replacement
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), encoded.len());
        let mut malformed = before.clone();
        malformed[start + position..start + position + changed.len()].copy_from_slice(&changed);
        let parsed = inspect_table_properties(
            &mut Cursor::new(&malformed),
            SCOPE,
            "Values",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            parsed
                .groups()
                .iter()
                .flat_map(|group| group.properties())
                .any(|property| {
                    property.name() == replacement
                        || property.text().is_ok_and(|value| value == replacement)
                })
        );
        assert!(
            image(&malformed)
                .prepare_retained_typed_field_rename(
                    "Values",
                    "ByteValue",
                    "Renamed",
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
    }
    let field = definition as usize * 4096 + 75 + 2 * 25;
    for (offset, bytes) in [(15, vec![7]), (16, vec![1]), (23, vec![2, 0])] {
        let mut malformed = before.clone();
        malformed[field + offset..field + offset + bytes.len()].copy_from_slice(&bytes);
        assert!(
            image(&malformed)
                .prepare_retained_typed_field_rename(
                    "Values",
                    "ByteValue",
                    "Renamed",
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
    }
    assert!(matches!(
        image(&before).prepare_retained_typed_field_rename(
            "Values",
            "ByteValue",
            "Renamed",
            &mut budget(),
            &|| true
        ),
        Err(FieldRenameError::Inspect(InspectError::Cancelled))
    ));
    for mut allowance in [
        ResourceBudget::new(0, 1_000_000, 500_000_000),
        ResourceBudget::new(1_000_000_000, 0, 500_000_000),
        ResourceBudget::new(1_000_000_000, 1_000_000, 0),
    ] {
        assert!(matches!(
            image(&before).prepare_retained_typed_field_rename(
                "Values",
                "ByteValue",
                "Renamed",
                &mut allowance,
                &|| false
            ),
            Err(FieldRenameError::Inspect(InspectError::Limit(_)))
                | Err(FieldRenameError::RowSource(crate::EditError::Inspect(
                    InspectError::Limit(_)
                )))
        ));
    }
}
