#[path = "mutation/fixture.rs"]
mod fixture;
mod support;

use std::io::Cursor;
use tablecraft_ace::{InspectError, inspect_table_features, read_table};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}

#[test]
fn calculation_and_presentation_declarations_are_inert_and_absence_stays_unknown() {
    for code in [5, 6] {
        let mut bytes = fixture::with_rule("Expression", "[Id]*2");
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        bytes[20] = code;
        bytes[10 * 4096 + 63 + 25 + 16] = 0xc0;
        let metadata = inspect_table_features(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let fields = metadata.columns().collect::<Vec<_>>();
        assert_eq!(fields[1].expression().unwrap(), Some("[Id]*2"));
        assert!(fields[1].calculated());
        assert_eq!(fields[1].append_only().unwrap(), None);
        assert_eq!(fields[1].result_native_type().unwrap(), None);
        assert!(!fields[0].calculated());
        assert_eq!(fields[0].expression().unwrap(), None);
        assert!(metadata.schema().columns()[1].scalar_type().is_none());
        assert_eq!(
            read_table(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );
    }
}

#[test]
fn complex_id_is_native_metadata_not_a_scalar_or_a_jet4_interpretation() {
    for code in [1, 5, 6] {
        let mut bytes = fixture::with_rule("Caption", "complex");
        if code != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        bytes[20] = code;
        bytes[10 * 4096 + 63 + 25] = 0x12;
        bytes[10 * 4096 + 63 + 25 + 11..10 * 4096 + 63 + 25 + 15]
            .copy_from_slice(&0x12345678_u32.to_le_bytes());
        let metadata = inspect_table_features(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            metadata.schema().columns()[1].complex_type_id(),
            if code == 1 { None } else { Some(0x12345678) }
        );
        assert!(metadata.schema().columns()[1].scalar_type().is_none());
    }
}

#[test]
fn wrong_typed_declarations_do_not_turn_into_defaults_and_unknown_flags_stay_raw() {
    let mut bytes = fixture::with_rule("AppendOnly", "true");
    bytes[10 * 4096 + 63 + 25 + 16] = 0x28;
    let metadata = inspect_table_features(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let field = metadata.columns().nth(1).unwrap();
    assert_eq!(field.append_only(), Err(InspectError::Unsupported));
    assert!(field.complex_foreign_key());
    assert!(!field.calculated());
    assert_eq!(field.column().extended_flags(), 0x28);
    assert!(field.column().scalar_type().is_none());
}

#[test]
fn features_obey_cancellation_and_shared_resource_limits() {
    let bytes = fixture::with_rule("Caption", "bounded");
    assert_eq!(
        inspect_table_features(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| true
        )
        .err(),
        Some(InspectError::Cancelled)
    );
    assert!(
        inspect_table_features(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        )
        .is_err()
    );
}
