use super::{Coverage, IndexProfile, NativeIndex};
use crate::{
    IndexInspection, InspectError, inspect_table_indexes,
    table::indexes::composite::{Columns, DecimalColumns},
    test_support::tuples::{decimal_fixture, numeric_fixture},
};
use decimal_fixture::Cell;
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::SchemaName;

fn metadata(bytes: &[u8]) -> IndexInspection {
    inspect_table_indexes(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000),
        &|| false,
    )
    .unwrap()
}

#[test]
fn direct_mutation_column_constructors_keep_complete_table_qualification() {
    let mut numeric = numeric_fixture::database(
        5,
        &[4, 4],
        &[(0, false), (1, true)],
        &[vec![Some(1), Some(2)]],
        0,
    );
    numeric_fixture::index_flags(&mut numeric, 0x82);
    let inspection = metadata(&numeric);
    let index = &inspection.indexes()[0];
    let schema = inspection.schema();
    assert_eq!(
        Columns::new(index, schema).err(),
        Some(InspectError::Unsupported)
    );
    assert_eq!(
        IndexProfile::new(index, schema).err(),
        Some(InspectError::Unsupported)
    );
    assert!(IndexProfile::for_check(index, schema).is_ok());

    let mut decimal = decimal_fixture::database(
        5,
        &[4, 16],
        &[(0, false), (1, true)],
        &[vec![Some(Cell::Number(1)), Some(Cell::Decimal(10001))]],
        0,
    );
    numeric_fixture::index_flags(&mut decimal, 0x82);
    let inspection = metadata(&decimal);
    let index = &inspection.indexes()[0];
    let schema = inspection.schema();
    assert_eq!(
        DecimalColumns::new(index, schema).err(),
        Some(InspectError::Unsupported)
    );
    assert_eq!(
        IndexProfile::new(index, schema).err(),
        Some(InspectError::Unsupported)
    );
    assert!(IndexProfile::for_check(index, schema).is_ok());
}

#[test]
fn physical_membership_changes_only_the_ignored_flag_not_known_or_foreign_constraints() {
    for flags in u8::MIN..=u8::MAX {
        let mut index = NativeIndex {
            name: SchemaName::new("Key".to_owned()).unwrap(),
            number: 0,
            data_number: 0,
            root: 1,
            usage: 0,
            kind: 0,
            flags,
            backing_primary: false,
            columns: Default::default(),
            reference: None,
            foreign_checked: false,
        };
        let known = flags & !0x8b == 0;
        assert_eq!(
            Coverage::PhysicalMembership.qualify(&index).is_ok(),
            known,
            "flags {flags:#x}"
        );
        assert_eq!(
            Coverage::CompleteTable.qualify(&index).is_ok(),
            known && flags & 0x02 == 0,
            "flags {flags:#x}"
        );
        index.kind = 2;
        assert_eq!(
            Coverage::PhysicalMembership.qualify(&index),
            Err(InspectError::Unsupported)
        );
        assert_eq!(
            Coverage::CompleteTable.qualify(&index),
            Err(InspectError::Unsupported)
        );
    }
}
