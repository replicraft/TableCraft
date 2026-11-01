//! A later rejected default request preserves the real earlier completed owner.
use super::{
    EditableTable, OfflineWriteSession, OmittingInsertableTable, SCOPE, budget, create, fs, sources,
};
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, Phase},
};
use tablecraft_ace::InspectError;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::InsertValue;
use tablecraft_io::{ExclusiveFile, IoError};

#[test]
fn later_resource_or_cancel_failure_preserves_default_commit_receipt_and_lease() {
    let source = sources::source(sources::Case::Seven);
    let (after, rows) = sources::inserted(&source, false).unwrap();
    for cancelled in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("created.accdb");
        let bundle = directory.path().join("journal");
        create(&path, &source);
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let staged = session
            .insert_table("Items", &mut budget(), &|| false)
            .unwrap()
            .plan_insert_with_omissions(
                vec![
                    InsertValue::Supplied(Value::Int32(24)),
                    InsertValue::Omitted,
                ],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(fs::read(&path).unwrap(), after);
        assert!(!bundle.exists());
        let inserter = session
            .insert_table("Items", &mut budget(), &|| false)
            .unwrap();
        let mut limited = if cancelled {
            budget()
        } else {
            ResourceBudget::new(1_000_000_000, 1_000_000, 0)
        };
        let failure = inserter
            .plan_insert_with_omissions(
                vec![
                    InsertValue::Supplied(Value::Int32(26)),
                    InsertValue::Omitted,
                ],
                &mut limited,
                &|| cancelled,
            )
            .err()
            .unwrap();
        let Cause::Editing(journal::Error::Native(NativeEditError::Inspect(error))) =
            failure.cause()
        else {
            panic!("expected the actual pre-plan native resource boundary");
        };
        if cancelled {
            assert!(matches!(error, InspectError::Cancelled));
        } else {
            assert!(
                matches!(error, InspectError::Limit(limit) if limit.resource == Resource::Steps)
            );
        }
        assert_eq!(failure.phase(), Phase::Planning);
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(failure.pair_state(), None);
        assert_eq!(fs::read(&path).unwrap(), after);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let editor = failure
            .into_session()
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(EditableTable::snapshot(&editor).rows(), rows);
        let session = editor.into_session();
        assert_eq!(session.last_completed(), Some(receipt));
        session.release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
