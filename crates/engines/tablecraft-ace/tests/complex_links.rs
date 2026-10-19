#[path = "mutation/fixture.rs"]
mod base;
#[path = "complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "complex_links/fixture.rs"]
mod links_fixture;
mod support;
use links_fixture::populated;

use std::io::Cursor;
use tablecraft_ace::{
    ComplexColumnLinks, ComplexReferenceViolation as V, InspectError, read_complex_column_links,
    read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16_384, 1_000_000)
}
fn read(bytes: &[u8]) -> Result<ComplexColumnLinks, InspectError> {
    read_complex_column_links(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
}
fn row_start(bytes: &[u8], page: usize, slot: usize) -> usize {
    page * 4096
        + u16::from_le_bytes(
            bytes[page * 4096 + 14 + 2 * slot..page * 4096 + 16 + 2 * slot]
                .try_into()
                .unwrap(),
        ) as usize
}

#[test]
fn parents_keep_scoped_keys_and_exact_child_rows_including_empty_membership() {
    for code in [2, 5, 6] {
        let mut bytes = populated();
        bytes[20] = code;
        let links = read(&bytes).unwrap();
        assert_eq!(
            links
                .parents()
                .iter()
                .map(|p| (p.key(), p.child_count()))
                .collect::<Vec<_>>(),
            [(Some(10), 2), (Some(20), 1), (Some(30), 0)]
        );
        assert_eq!(
            links
                .child_rows(0)
                .unwrap()
                .map(|row| row[1].clone())
                .collect::<Vec<_>>(),
            [Value::Int32(100), Value::Int32(101)]
        );
        assert_eq!(
            links.child_rows(1).unwrap().next().unwrap()[2],
            Value::Int32(3)
        );
        assert_eq!(links.child_rows(2).unwrap().len(), 0);
        assert!(links.child_rows(3).is_none());
        assert_eq!(links.child_snapshot().rows().len(), 3);
        assert_eq!(links.column().type_id(), 7);
        assert_eq!(
            read_table(
                &mut Cursor::new(&bytes),
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
fn orphans_duplicate_parents_duplicate_identities_and_null_child_keys_fail_atomically() {
    let original = populated();
    for case in 0..5 {
        let mut bytes = original.clone();
        let parent = row_start(&bytes, 12, 1);
        let child = row_start(&bytes, 22, 1);
        let expected = match case {
            0 => {
                support::put32(&mut bytes, parent + 6, 10);
                V::DuplicateParent
            }
            1 => {
                support::put32(&mut bytes, child + 2, 999);
                V::MissingParent
            }
            2 => {
                support::put32(&mut bytes, child + 10, 1);
                V::DuplicateChild
            }
            3 => {
                bytes[child + 14] &= !1;
                V::NullChild
            }
            _ => {
                bytes[child + 14] &= !4;
                V::NullChild
            }
        };
        assert_eq!(
            read(&bytes).err(),
            Some(InspectError::ComplexReference(expected))
        );
    }
}

#[test]
fn absent_parent_keys_stay_null_and_unqualified_profiles_flags_and_keys_stay_unsupported() {
    let original = populated();
    let mut bytes = original.clone();
    let last = row_start(&bytes, 12, 2);
    bytes[last + 10] &= !2;
    let links = read(&bytes).unwrap();
    assert_eq!(links.parents()[2].key(), None);
    assert_eq!(links.parents()[2].child_count(), 0);
    for (offset, value) in [
        (20, 3),
        (10 * 4096 + 63 + 25 + 15, 3),
        (10 * 4096 + 63 + 25 + 16, 0x40),
    ] {
        let mut bytes = original.clone();
        bytes[offset] = value;
        assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));
    }
    let mut bytes = original;
    let first = row_start(&bytes, 12, 0);
    support::put32(&mut bytes, first + 6, 0);
    assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));
    assert_eq!(
        read(&base::with_rule("Caption", "ordinary scalar")).err(),
        Some(InspectError::Unsupported)
    );
}

#[test]
fn scoped_storage_views_retain_unknown_column_limits_and_cancellation() {
    let bytes = populated();
    assert_eq!(
        read_complex_column_links(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "missing",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::UnknownColumn)
    );
    for (mut allowance, cancelled) in [(budget(), true), (ResourceBudget::new(66, 1, 100), false)] {
        assert!(
            read_complex_column_links(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Score",
                &mut allowance,
                &|| cancelled
            )
            .is_err()
        );
    }
}

#[test]
fn empty_and_all_absent_parent_storage_preserve_distinct_snapshot_shapes() {
    let mut bytes = populated();
    support::put32(&mut bytes, 20 * 4096 + 16, 0);
    bytes[22 * 4096..23 * 4096].copy_from_slice(&support::data_page(20, &[]));
    for slot in 0..3 {
        let start = row_start(&bytes, 12, slot);
        bytes[start + 10] &= !2;
    }
    let links = read(&bytes).unwrap();
    assert_eq!(links.parents().len(), 3);
    assert!(
        links
            .parents()
            .iter()
            .all(|parent| parent.key().is_none() && parent.child_count() == 0)
    );
    assert_eq!(links.child_rows(0).unwrap().len(), 0);
    support::put32(&mut bytes, 10 * 4096 + 16, 0);
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &[]));
    let links = read(&bytes).unwrap();
    assert!(links.parents().is_empty());
    assert!(links.child_rows(0).is_none());
}
