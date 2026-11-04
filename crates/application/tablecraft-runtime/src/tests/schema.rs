use super::{open, request};
use crate::{Command, Error, Response, Worker};
use tablecraft_ace::InspectError;
use tablecraft_testkit::fixtures::access::{complex_schema, mutation_fixture};

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
    let original = complex_schema::database();
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
    assert_eq!(field.native_type, 0x12);
    assert_eq!(field.storage_bytes, 4);
    assert_eq!(field.flags, 3);
    assert_eq!(field.extended_flags, 0);
    assert_eq!(field.scalar_type, None);
    assert_eq!(field.reader_nullable, None);
    assert_eq!(field.complex_type_id, Some(7));
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
