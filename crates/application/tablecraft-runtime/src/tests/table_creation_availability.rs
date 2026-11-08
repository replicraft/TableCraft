//! Ordinary opens publish exact source eligibility without authoring any object.
use super::{open, request};
use crate::{Response, Worker};
use tablecraft_testkit::fixtures::access::{
    canonical_add_table, canonical_blank_creation,
    canonical_initial_table_creation::{self, Case},
    canonical_repeated_add_table,
};

#[test]
fn ordinary_open_advertises_only_qualified_table_creation_profiles_without_writes() {
    let blank = canonical_blank_creation::source();
    let mut modified = blank.clone();
    *modified.last_mut().unwrap() ^= 1;
    let populated = canonical_initial_table_creation::source(Case::IdOnly).bytes;
    let supported = cfg!(all(
        feature = "offline-edit",
        feature = "ace16-create",
        any(target_os = "macos", target_os = "linux", windows)
    ));
    for (bytes, expected, user_tables) in [
        (blank, supported, 0),
        (modified, false, 0),
        (populated, supported, 1),
        (canonical_add_table::source().before, supported, 1),
        (canonical_add_table::source().after, supported, 2),
        (canonical_repeated_add_table::source().before, supported, 2),
        (
            canonical_repeated_add_table::with_existing_tables(18).before,
            supported,
            18,
        ),
        (canonical_repeated_add_table::capacity_source(), false, 19),
        (
            canonical_add_table::claimed_append_destination(false),
            false,
            1,
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ordinary.accdb");
        std::fs::write(&path, &bytes).unwrap();
        let worker = Worker::new(|| {});
        let Response::Opened {
            table_creation_available,
            table_design_available,
            document_revision,
            source_path,
            tables,
            ..
        } = request(&worker, open(&path, "")).unwrap()
        else {
            panic!("expected ordinary native open");
        };
        assert_eq!(table_creation_available, expected);
        assert_eq!(table_design_available, expected);
        assert!(document_revision > 0);
        assert_eq!(source_path, path);
        assert_eq!(
            tables.iter().filter(|table| !table.system).count(),
            user_tables
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
