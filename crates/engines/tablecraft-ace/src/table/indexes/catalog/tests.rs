use super::*;
use crate::table::indexes::IndexColumn;
use tablecraft_schema::SchemaName;

fn index(columns: &[(usize, bool)], flags: u8) -> NativeIndex {
    NativeIndex {
        name: SchemaName::new("Key".into()).unwrap(),
        number: 0,
        data_number: 0,
        root: 1,
        usage: 1,
        kind: 0,
        flags,
        backing_primary: false,
        columns: columns
            .iter()
            .map(|&(number, descending)| IndexColumn { number, descending })
            .collect(),
        reference: None,
        foreign_checked: false,
    }
}
#[test]
fn one_to_one_unique_keys_are_independent_of_physical_direction_and_order() {
    let key = index(&[(0, false), (1, false)], 0x80);
    for columns in [&[(0, false), (1, true)][..], &[(1, true), (0, true)][..]] {
        assert!(
            unique_key(
                &key,
                &[index(columns, 0x81)],
                &mut ResourceBudget::new(0, 0, 100),
                &|| false
            )
            .unwrap()
        );
    }
    for other in [
        index(&[(0, false), (1, false)], 0x80),
        index(&[(0, false)], 0x81),
        index(&[(0, false), (2, false)], 0x81),
        index(&[(0, false), (1, false)], 0xc1),
    ] {
        assert!(
            !unique_key(&key, &[other], &mut ResourceBudget::new(0, 0, 100), &|| {
                false
            })
            .unwrap()
        );
    }
    assert!(
        unique_key(
            &key,
            &[index(&[(0, false), (1, true)], 0x81)],
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        )
        .is_err()
    );
    assert!(
        unique_key(
            &key,
            &[index(&[(0, false), (1, true)], 0x81)],
            &mut ResourceBudget::new(0, 0, 100),
            &|| true
        )
        .is_err()
    );
}
