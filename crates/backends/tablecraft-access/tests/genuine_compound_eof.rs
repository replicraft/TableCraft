#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::OfflineWriteSession;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{EditableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::genuine_compound_eof as sources;

const SCOPE: CatalogScope = CatalogScope::new(152);
const SQL: &str =
    "UPDATE Sentinel SET Marker=IIf([ID]=23,'aaaaaaaaaaaaaaaaaaa','ccccccccccccccccccc')";

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(bytes: &[u8], expected: &str) {
    let actual = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn rows(first: &str, second: &str) -> Vec<Vec<Value>> {
    [(23, first), (24, second)]
        .into_iter()
        .map(|(id, marker)| {
            vec![
                Value::Int32(id),
                Value::Text(marker.into()),
                Value::Binary(vec![199, 0, 255]),
            ]
        })
        .collect()
}

#[test]
#[ignore = "requires all four source-qualified compound owned-plus-EOF images"]
fn compound_owned_and_eof_sql_session_commits_two_original_rows_atomically() {
    let cases = sources::load_all(verify);
    let mut saves = 0;
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Sentinel", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
        assert_eq!(
            editor.snapshot().rows(),
            rows("unchanged sentinel", "unchanged sentinel")
        );
        let query = parse_query(SQL, ParseLimits::default()).unwrap();
        let staged = editor
            .into_session()
            .plan_update(query.query(), &mut budget(), None, &|| false)
            .unwrap();
        assert_eq!((staged.matched_rows(), staged.changed_rows()), (2, 2));
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(receipt.matched_rows(), Some(2));
        assert_eq!(receipt.changed_rows(), Some(2));
        assert_eq!(session.last_completed(), Some(receipt));
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let editor = session
            .edit_table("Sentinel", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            editor.snapshot().rows(),
            rows("aaaaaaaaaaaaaaaaaaa", "ccccccccccccccccccc")
        );
        editor.into_session().release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 2);
}
