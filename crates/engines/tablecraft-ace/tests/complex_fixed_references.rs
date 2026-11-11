//! ACE 2010 fixed references qualify only inside a fully resolved complex owner.
use std::io::Cursor;
use tablecraft_ace::{
    InspectError, TableSnapshotLimits, VersionHistoryReader, read_attachment_column,
    read_multivalue_column, read_table, read_table_projection_batched, read_version_history_column,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{attachment_modern, complex_links, version_history};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 2_000_000)
}

fn source() -> Vec<u8> {
    let mut bytes = version_history::database();
    bytes[20] = 3;
    bytes
}

#[test]
fn ace2010_fixed_references_keep_history_and_multivalue_values_scoped() {
    let bytes = source();
    let history = read_version_history_column(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "Notes",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        history
            .links()
            .parents()
            .iter()
            .map(|parent| parent.child_count())
            .collect::<Vec<_>>(),
        [2, 2, 0]
    );
    assert_eq!(
        history.member_value(0, 0, "Notes"),
        Some(&Value::Text("é🦀\0older".into()))
    );
    let mut cursor = Cursor::new(&bytes);
    let mut allowance = budget();
    let mut reader = VersionHistoryReader::open(
        &mut cursor,
        CatalogScope::new(1),
        "Items",
        "Notes",
        TableSnapshotLimits::default(),
        &mut allowance,
        &|| false,
    )
    .unwrap();
    assert_eq!(reader.read_parent(0).unwrap().stored_versions().len(), 2);
    assert_eq!(reader.read_parent(2).unwrap().stored_versions().len(), 0);
    assert!(matches!(
        read_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Children",
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
    let scalars = read_table_projection_batched(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &["Id"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        scalars.rows(),
        [
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Int32(3)]
        ]
    );
    let mut values = complex_links::populated();
    values[20] = 3;
    let values = read_multivalue_column(
        &mut Cursor::new(values),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        values.values(0).unwrap().collect::<Vec<_>>(),
        [&Value::Int32(100), &Value::Int32(101)]
    );
}

#[test]
fn fixed_reference_width_flags_extension_and_aliases_remain_closed() {
    let foreign = 20 * 4096 + 63;
    let identity = foreign + 3 * 25;
    for (case, offset, replacement) in [
        ("foreign width", foreign + 23, vec![8, 0]),
        ("foreign flags", foreign + 15, vec![1]),
        ("identity flags", identity + 15, vec![5]),
        ("foreign extension", foreign + 16, vec![0]),
        ("physical alias", identity + 21, vec![0, 0]),
    ] {
        let mut bytes = source();
        bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
        let failure = read_version_history_column(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            "Notes",
            &mut budget(),
            &|| false,
        )
        .err();
        assert!(
            matches!(failure, Some(InspectError::Unsupported)),
            "{case}: {failure:?}"
        );
    }
    let mut bytes = source();
    bytes[identity + 5..identity + 7].copy_from_slice(&0_u16.to_le_bytes());
    let failure = read_version_history_column(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Notes",
        &mut budget(),
        &|| false,
    )
    .err();
    assert!(
        matches!(failure, Some(InspectError::Malformed)),
        "duplicate native ordinal: {failure:?}"
    );
}

#[test]
fn ace2010_attachment_payload_language_is_not_promoted_by_fixed_references() {
    assert!(matches!(
        read_attachment_column(
            &mut Cursor::new(attachment_modern::database(3)),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
}
