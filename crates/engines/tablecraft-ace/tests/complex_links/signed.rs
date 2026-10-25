use super::{InspectError, V, populated, read, row_start, support};
use tablecraft_core::values::Value;

const CHILD_PAGE: usize = 22;
const PARENT_PAGE: usize = 12;
const FOREIGN_KEY_OFFSET: usize = 2;
const IDENTITY_OFFSET: usize = 10;
const PARENT_KEY_OFFSET: usize = 6;

#[test]
fn child_identities_preserve_signed_boundaries_and_stored_order() {
    for code in [2, 5, 6] {
        for identities in [[i32::MIN, -1, 0], [-7, -2, i32::MAX]] {
            let mut bytes = populated();
            bytes[20] = code;
            for (slot, identity) in identities.into_iter().enumerate() {
                let start = row_start(&bytes, CHILD_PAGE, slot);
                support::put32(&mut bytes, start + IDENTITY_OFFSET, identity as u32);
            }
            let links = read(&bytes).unwrap();
            let observed = (0..2)
                .flat_map(|parent| links.child_rows(parent).unwrap())
                .map(|row| row[2].clone())
                .collect::<Vec<_>>();
            assert_eq!(observed, identities.map(Value::Int32));
        }
    }
}

#[test]
fn signed_identity_duplicates_fail_but_parent_and_foreign_keys_stay_positive() {
    for identity in [i32::MIN, -1, 0, i32::MAX] {
        let mut bytes = populated();
        for slot in [0, 2] {
            let start = row_start(&bytes, CHILD_PAGE, slot);
            support::put32(&mut bytes, start + IDENTITY_OFFSET, identity as u32);
        }
        assert_eq!(
            read(&bytes).err(),
            Some(InspectError::ComplexReference(V::DuplicateChild))
        );
    }
    for invalid in [i32::MIN, -1, 0] {
        let mut bytes = populated();
        let parent = row_start(&bytes, PARENT_PAGE, 0);
        support::put32(&mut bytes, parent + PARENT_KEY_OFFSET, invalid as u32);
        assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));

        let mut bytes = populated();
        let child = row_start(&bytes, CHILD_PAGE, 0);
        support::put32(&mut bytes, child + FOREIGN_KEY_OFFSET, invalid as u32);
        assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));
    }
}
