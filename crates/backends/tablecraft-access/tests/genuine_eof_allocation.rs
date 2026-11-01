#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{fs, io::Cursor};
use tablecraft_access::editing::offline::OfflineWriteSession;
use tablecraft_ace::{
    NativeFittedIntentGroup, NativeFittedUpdates, NativeImage, NativeUpdateProfile,
    SlotScalarUpdate,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::genuine_eof_allocation::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(151);

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

fn rows(marker: &str) -> Vec<Vec<Value>> {
    vec![vec![
        Value::Int32(23),
        Value::Text(marker.into()),
        Value::Binary(vec![199, 0, 255]),
    ]]
}

fn require_original_request_profile(case: &Case) {
    let image = NativeImage::capture(
        &mut Cursor::new(&case.before),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let group = NativeFittedIntentGroup::new(
        "Marker",
        NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
            0,
            Value::Text(sources::OLD.into()),
            Value::Text(sources::GROWN.into()),
        )]),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let plan = image
        .plan_recoverable_ordinary_variable_updates("Sentinel", vec![group], &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(
        plan.intent().profile(),
        NativeUpdateProfile::Ace16OrdinaryEofAllocation
    );
    assert!(plan.intent().owned_growth().is_none());
    assert!(plan.intent().fitted().is_none());
    assert_eq!(plan.intent().eof_allocation().unwrap().groups().len(), 1);
    assert_eq!(
        &plan.intent().encode(&mut budget(), &|| false).unwrap()[..8],
        b"TCNI0007"
    );
    assert_eq!(plan.before_bytes(), case.before);
    assert_eq!(plan.after_bytes(), case.after);
    assert_eq!(plan.after_bytes().len(), plan.before_bytes().len() + 4096);
}

#[test]
#[ignore = "requires all four pinned genuine EOF-allocation SOURCE images"]
fn first_eof_allocation_typed_and_sql_sessions_commit_exact_images_and_retain_lease() {
    let cases = sources::load_all(verify);
    // Both original-request profile decisions are checked before any target
    // file exists. These plans are assertions, never expected-byte authorities.
    for case in &cases {
        require_original_request_profile(case);
    }
    let mut saves = 0;
    for case in cases {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let staged = if typed {
                let editor = session
                    .edit_table("Sentinel", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                assert_eq!(editor.snapshot().rows(), rows(sources::OLD));
                let handle = editor.snapshot().row_handle(0).unwrap();
                editor
                    .plan_updates(
                        "Marker",
                        &[CellUpdate::new(handle, Value::Text(sources::GROWN.into()))],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
            } else {
                let sql = format!(
                    "UPDATE Sentinel SET Marker='{}' WHERE [ID]=23",
                    sources::GROWN
                );
                let query = parse_query(&sql, ParseLimits::default()).unwrap();
                session
                    .plan_update(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.changed_rows(), Some(1));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let editor = session
                .edit_table("Sentinel", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.snapshot().rows(), rows(sources::GROWN));
            editor.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves += 1;
        }
    }
    assert_eq!(saves, 4);
}
