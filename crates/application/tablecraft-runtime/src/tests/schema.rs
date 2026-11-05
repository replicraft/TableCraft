use super::{open, request};
use crate::{Command, Error, FieldPropertyValue, Response, Worker};
use tablecraft_ace::InspectError;
use tablecraft_testkit::fixtures::access::{
    complex_schema, field_presentation, mutation_fixture, wide_properties,
};

#[test]
fn public_describe_preserves_original_names_widths_and_reader_policies() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fields.mdb");
    let original = mutation_fixture::database();
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    assert!(matches!(
        request(
            &worker,
            Command::Describe {
                table: "Items".to_owned()
            }
        ),
        Err(Error::NoDatabase)
    ));
    request(&worker, open(&path, "")).unwrap();
    let Response::Details {
        table,
        declared_rows,
        fields,
    } = request(
        &worker,
        Command::Describe {
            table: "Items".to_owned(),
        },
    )
    .unwrap()
    else {
        panic!("expected field declarations");
    };
    assert_eq!(table, "Items");
    assert_eq!(declared_rows, 3);
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["Id", "Score"]
    );
    for (number, field) in fields.iter().enumerate() {
        assert_eq!(field.number, number);
        assert_eq!(field.native_type, 4);
        assert_eq!(field.storage_bytes, 4);
        assert!(field.fixed);
        assert_eq!(field.flags, 3);
        assert_eq!(field.extended_flags, 0);
        assert_eq!(field.scalar_type.as_deref(), Some("Int32"));
        assert_eq!(field.reader_nullable, Some(true));
        assert_eq!(field.decimal_parameters, None);
        assert_eq!(field.complex_type_id, None);
    }
    assert!(matches!(
        request(
            &worker,
            Command::Describe {
                table: "Missing".to_owned()
            }
        ),
        Err(Error::Native(InspectError::UnknownTable))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn describe_retains_unsupported_fields_without_claiming_readability() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("complex.accdb");
    let mut original = complex_schema::database();
    // Stored collection browsing can now admit qualified complex declarations.
    // This remains a genuinely unknown native declaration, not an obsolete
    // scalar-only refusal of a newly supported field family.
    original[10 * 4096 + 63 + 25] = 0xff;
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    request(&worker, open(&path, "")).unwrap();
    let Response::Details { table, fields, .. } = request(
        &worker,
        Command::Describe {
            table: "Items".to_owned(),
        },
    )
    .unwrap() else {
        panic!("expected retained field declarations");
    };
    assert_eq!(table, "Items");
    assert_eq!(fields.len(), 2);
    let field = &fields[1];
    assert_eq!(field.name, "Score");
    assert_eq!(field.number, 1);
    assert_eq!(field.native_type, 0xff);
    assert_eq!(field.storage_bytes, 4);
    assert_eq!(field.flags, 3);
    assert_eq!(field.extended_flags, 0);
    assert_eq!(field.scalar_type, None);
    assert_eq!(field.reader_nullable, None);
    assert_eq!(field.complex_type_id, None);
    assert!(matches!(
        request(
            &worker,
            Command::Page {
                table: "Items".to_owned(),
                offset: 0
            }
        ),
        Err(Error::Native(InspectError::Unsupported))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn describe_retains_real_property_values_without_reading_rows_or_evaluating_text() {
    let mut base = field_presentation::database(
        &[
            wide_properties::names(&[
                "Required",
                "AllowZeroLength",
                "DefaultValue",
                "ValidationRule",
                "ValidationText",
                "Description",
                "Caption",
            ]),
            field_presentation::score_group(2, &[field_presentation::property(0, 1, 1, &[0])]),
            field_presentation::score_group(
                1,
                &[
                    field_presentation::property(0, 1, 1, &[0xff]),
                    field_presentation::property(1, 1, 1, &[0]),
                    field_presentation::property(
                        2,
                        1,
                        12,
                        &wide_properties::text("RunCode(\"do not execute\")"),
                    ),
                    field_presentation::property(3, 1, 10, &wide_properties::text(">0")),
                    field_presentation::property(4, 1, 10, &wide_properties::text("Keep positive")),
                    field_presentation::property(5, 1, 10, &wide_properties::text("Résumé")),
                    field_presentation::property(6, 1, 12, &wide_properties::text("")),
                ],
            ),
        ],
        false,
    );
    // Describe must remain useful even when user records cannot be read.
    base[12 * 4096] = 0;

    for (file_name, ace16_family, expected_format) in [
        ("properties.mdb", false, "Jet4"),
        ("properties.accdb", true, "Ace2019"),
    ] {
        let mut original = base.clone();
        if ace16_family {
            // Literal ACE 16-family header shape, independently authored like
            // the saved-query fixture; this is not a native Access save.
            original[4..20].copy_from_slice(b"Standard ACE DB\0");
            original[20] = 6;
        }

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(file_name);
        std::fs::write(&path, &original).unwrap();
        let worker = Worker::new(|| {});
        let Response::Opened { format, .. } = request(&worker, open(&path, "")).unwrap() else {
            panic!("expected native profile inventory");
        };
        assert_eq!(format, expected_format);

        let Response::Details { fields, .. } = request(
            &worker,
            Command::Describe {
                table: "Items".into(),
            },
        )
        .unwrap() else {
            panic!("expected native metadata");
        };
        assert!(
            fields[0]
                .properties
                .iter()
                .all(|property| property.value == FieldPropertyValue::Absent)
        );
        let observed = fields[1]
            .properties
            .iter()
            .map(|property| (property.name, property.value.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            observed,
            [
                ("Required", FieldPropertyValue::Boolean(true)),
                ("AllowZeroLength", FieldPropertyValue::Boolean(false)),
                (
                    "DefaultValue",
                    FieldPropertyValue::Text("RunCode(\"do not execute\")".into())
                ),
                ("ValidationRule", FieldPropertyValue::Text(">0".into())),
                (
                    "ValidationText",
                    FieldPropertyValue::Text("Keep positive".into())
                ),
                ("Description", FieldPropertyValue::Text("Résumé".into())),
                ("Caption", FieldPropertyValue::Text("".into())),
            ]
        );
        assert_eq!(fields[1].reader_nullable, Some(true));
        assert!(
            request(
                &worker,
                Command::Page {
                    table: "Items".into(),
                    offset: 0,
                },
            )
            .is_err()
        );
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}
