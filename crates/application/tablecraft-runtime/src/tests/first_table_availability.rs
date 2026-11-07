//! Ordinary opens publish exact source eligibility without authoring any object.
use super::{open, request};
use crate::{Response, Worker};
use tablecraft_testkit::fixtures::access::{
    canonical_blank_creation,
    canonical_initial_table_creation::{self, Case},
};

#[test]
fn ordinary_open_advertises_first_table_only_for_the_exact_editable_canonical_snapshot() {
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
        (populated, false, 1),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ordinary.accdb");
        std::fs::write(&path, &bytes).unwrap();
        let worker = Worker::new(|| {});
        let Response::Opened {
            first_table_creation_available,
            document_revision,
            source_path,
            tables,
            ..
        } = request(&worker, open(&path, "")).unwrap()
        else {
            panic!("expected ordinary native open");
        };
        assert_eq!(first_table_creation_available, expected);
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
