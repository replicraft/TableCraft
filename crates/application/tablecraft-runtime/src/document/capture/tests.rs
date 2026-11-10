use super::read;
use crate::Error;
use std::{
    cell::Cell,
    fs::File,
    io::{Cursor, Write},
};
use tablecraft_access::AccessInputError;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::fixtures::access::mutation_fixture;

#[test]
fn capture_uses_caller_resources_and_cancellation_without_returning_a_prefix() {
    let mut bytes = mutation_fixture::database();
    bytes.resize(2 * 1024 * 1024, 0);
    assert!(
        read(
            &mut Cursor::new(&bytes),
            None,
            &mut ResourceBudget::new(64, 0, 1_000_000),
            &|| false
        )
        .is_err()
    );
    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() > 20
    };
    let mut source = Cursor::new(&bytes);
    assert!(
        read(
            &mut source,
            None,
            &mut ResourceBudget::new(u64::MAX, 0, u64::MAX),
            &cancelled
        )
        .is_err()
    );
    assert!(calls.get() > 20);
    assert!(source.position() < bytes.len() as u64);
    assert_eq!(
        read(
            &mut Cursor::new(&bytes),
            None,
            &mut ResourceBudget::new(u64::MAX, 0, u64::MAX),
            &|| false
        )
        .unwrap(),
        bytes
    );
}

#[test]
fn native_geometry_rejects_an_oversized_sparse_source_before_body_allocation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("outside-native-envelope.mdb");
    let mut file = File::create(&path).unwrap();
    file.write_all(&mutation_fixture::database()).unwrap();
    file.set_len(2 * 1024 * 1024 * 1024 + 4096).unwrap();
    drop(file);
    let mut file = File::open(path).unwrap();
    assert!(matches!(
        read(
            &mut file,
            None,
            &mut ResourceBudget::new(4096, 0, 100),
            &|| false
        ),
        Err(Error::Input(AccessInputError::Native(
            InspectError::StructureLimit
        )))
    ));
}

#[test]
fn guarded_capture_keeps_exact_witness_precedence_over_changed_source_format() {
    let original = mutation_fixture::database();
    for position in [0, original.len() - 1] {
        let mut changed = original.clone();
        changed[position] ^= 0xff;
        assert!(matches!(
            read(
                &mut Cursor::new(changed),
                Some(&original),
                &mut ResourceBudget::new(u64::MAX, 0, u64::MAX),
                &|| false
            ),
            Err(Error::SourceChanged)
        ));
    }
    assert!(matches!(
        read(
            &mut Cursor::new(&original[..4]),
            Some(&original),
            &mut ResourceBudget::new(u64::MAX, 0, u64::MAX),
            &|| false
        ),
        Err(Error::SourceChanged)
    ));
    assert_eq!(
        read(
            &mut Cursor::new(&original),
            Some(&original),
            &mut ResourceBudget::new(u64::MAX, 0, u64::MAX),
            &|| false
        )
        .unwrap(),
        original
    );
}
