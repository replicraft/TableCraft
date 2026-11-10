//! Independent raw reference carriers stay confined to resolved complex readers.
use std::io::Cursor;
use tablecraft_ace::{
    InspectError, MultivalueReader, TableSnapshotLimits, VersionHistoryReader,
    inspect_complex_schema, read_multivalue_column, read_table, read_version_history_column,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{complex_links, version_history};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16_384, 2_000_000)
}

#[test]
fn variable_multivalue_references_preserve_values_through_complete_and_parent_reads() {
    for code in [2, 3, 5, 6] {
        let mut bytes = complex_links::variable::populated();
        bytes[20] = code;
        let complete = read_multivalue_column(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false,
        )
        .unwrap_or_else(|error| panic!("profile code {code}: {error:?}"));
        assert_eq!(
            complete.values(0).unwrap().cloned().collect::<Vec<_>>(),
            [Value::Int32(100), Value::Int32(101)]
        );
        assert_eq!(
            complete.values(1).unwrap().cloned().collect::<Vec<_>>(),
            [Value::Int32(200)]
        );
        assert_eq!(complete.values(2).unwrap().len(), 0);
        let mut source = Cursor::new(&bytes);
        let mut allowance = budget();
        let mut reader = MultivalueReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .unwrap();
        assert_eq!(
            reader.read_parent(0).unwrap().values(),
            [Value::Int32(100), Value::Int32(101)]
        );
        assert_eq!(reader.read_parent(1).unwrap().values(), [Value::Int32(200)]);
        assert!(reader.read_parent(2).unwrap().values().is_empty());
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
    }
}

#[test]
fn variable_history_references_retain_stored_order_text_nulls_and_date_bits() {
    for (mut bytes, code) in [
        version_history::variable::database,
        version_history::variable::variable_modified,
    ]
    .into_iter()
    .flat_map(|fixture| [2, 3, 5, 6].map(|code| (fixture(), code)))
    {
        bytes[20] = code;
        let history = read_version_history_column(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Notes",
            &mut budget(),
            &|| false,
        )
        .unwrap_or_else(|error| panic!("profile code {code}: {error:?}"));
        let first = history
            .stored_versions(0)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            first.iter().map(|row| row.identity()).collect::<Vec<_>>(),
            [2, 9]
        );
        assert_eq!(first[0].value(), Some("é🦀\0older"));
        assert_eq!(
            first[0].modified().unwrap().to_bits(),
            (-0.25_f64).to_bits()
        );
        let mut source = Cursor::new(&bytes);
        let mut allowance = budget();
        let mut reader = VersionHistoryReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Notes",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .unwrap();
        let first = reader.read_parent(0).unwrap();
        assert_eq!(
            first.stored_versions().next().unwrap().unwrap().value(),
            Some("é🦀\0older")
        );
        let second = reader.read_parent(1).unwrap();
        let values = second
            .stored_versions()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(values[0].value(), Some(""));
        assert_eq!(values[0].modified().unwrap().to_bits(), 1);
        assert_eq!(values[1].value(), None);
        assert_eq!(values[1].modified(), None);
        assert_eq!(reader.read_parent(2).unwrap().stored_versions().len(), 0);
    }
}

#[test]
fn variable_reference_roles_refuse_unknown_flags_mixed_storage_and_wrong_widths() {
    for (offset, value) in [
        (63 + 15, 3),
        (63 + 16, 0),
        (63 + 23, 8),
        (63 + 50 + 15, 7),
        (63 + 50 + 16, 0),
    ] {
        let mut bytes = complex_links::variable::populated();
        bytes[20 * 4096 + offset] = value;
        assert!(
            inspect_complex_schema(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
}

#[test]
fn variable_reference_payloads_require_exact_nonnull_int32_and_unique_identities() {
    for corruption in 0..3 {
        let mut bytes = complex_links::variable::populated();
        let start = usize::from(
            u16::from_le_bytes(bytes[22 * 4096 + 14..22 * 4096 + 16].try_into().unwrap()) & 0x1fff,
        );
        let row = 22 * 4096 + start;
        match corruption {
            0 => bytes[row + 16] = 9,
            1 => bytes[row + 22] &= !1,
            _ => bytes[row + 10..row + 14].copy_from_slice(&2_i32.to_le_bytes()),
        }
        assert!(
            read_multivalue_column(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Score",
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
    }
}

#[test]
fn variable_history_modified_members_reject_unknown_flags_and_non_date_widths() {
    for (template_flags, flat_flags, width) in [(0x12, 0x02, 8), (0x02, 0x12, 8), (0x02, 0x02, 7)] {
        let mut bytes = version_history::variable::variable_modified();
        let template = 17 * 4096 + 63 + 25;
        let flat = 20 * 4096 + 63 + 50;
        bytes[template + 15] = template_flags;
        bytes[flat + 15] = flat_flags;
        bytes[template + 23..template + 25].copy_from_slice(&u16::to_le_bytes(width));
        bytes[flat + 23..flat + 25].copy_from_slice(&u16::to_le_bytes(width));
        assert!(matches!(
            inspect_complex_schema(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            ),
            Err(InspectError::Unsupported)
        ));
    }
}
