//! Child of the root-owned mixed session test; no duplicate storage/controller.
use super::{SCOPE, budget, locked};
use std::{env, fs, io::Cursor, path::Path};
use tablecraft_access::editing::{
    NativeEditError,
    journal::{self, NativeJournalEditor},
    offline::{Cause, OfflineWriteSession, Phase, SessionState},
};
use tablecraft_ace::NativeImage;
use tablecraft_core::values::Value;
use tablecraft_db::{CellUpdate, EditableTable, SnapshotError, UpdateProfile};
use tablecraft_io::journal::RecoveryOutcome;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_mixed_before_change::{self as model, Recipe};

#[test]
fn mixed_handles_are_snapshot_local_after_commit_and_foreign_capture() {
    for header in [5, 6] {
        let case = model::source(header, Recipe::CounterPrimary);
        let expected = case.expected.as_ref().unwrap();
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("target.accdb");
        fs::write(&database, &case.before).unwrap();
        let session =
            OfflineWriteSession::open(&database, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let stale = editor.snapshot().row_handle(0).unwrap();
        let staged = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(stale, Value::Int32(17))],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (mut session, receipt) = staged
            .apply(&root.path().join("journal"), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(fs::read(&database).unwrap(), *expected);

        let foreign_image = NativeImage::capture(
            &mut Cursor::new(expected),
            CatalogScope::new(303),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let foreign =
            NativeJournalEditor::new(foreign_image, "Items", &mut budget(), &|| false).unwrap();
        let foreign_handle = foreign.snapshot().row_handle(0).unwrap();
        for handle in [stale, foreign_handle] {
            let editor = session
                .edit_table("Items", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.update_profile(), UpdateProfile::ExistingInt32);
            assert_eq!(
                editor.snapshot().resolve(handle),
                Err(SnapshotError::StaleRow)
            );
            let error = editor
                .plan_updates(
                    "Score",
                    &[CellUpdate::new(handle, Value::Int32(29))],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .expect("foreign or stale handle cannot produce a native request");
            assert_eq!(error.phase(), Phase::Planning);
            assert!(matches!(
                error.cause(),
                Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                    SnapshotError::StaleRow
                )))
            ));
            assert_eq!(error.state(), SessionState::Ready);
            assert_eq!(error.last_completed(), Some(receipt));
            assert_eq!(error.current_outcome(), None);
            assert_eq!(error.pair_state(), None);
            assert_eq!(fs::read(&database).unwrap(), *expected);
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            locked(&database);
            session = error.into_session();
        }
        session.release().unwrap();
    }
}

#[test]
#[ignore = "explicit frozen rich SOURCE export directory; no fixture graph copy"]
fn projected_journal_editor_uses_actual_rich_source_and_two_literal_full_images() {
    let directory = env::var_os("TABLECRAFT_MIXED_RICH_SOURCE_DIR")
        .expect("explicit frozen rich source export");
    let directory = Path::new(&directory);
    assert!(directory.is_absolute());
    for (id, final_value) in [("rich-sequential", 1018_i32), ("rich-counter-read", 1_i32)] {
        let before_path = directory.join(format!("h6-{id}-original.accdb"));
        let expected_path = directory.join(format!("h6-{id}-expected.accdb"));
        let before = fs::read(&before_path).unwrap();
        let expected = fs::read(&expected_path).unwrap();
        assert_eq!(before.len(), 32 * 4096);
        assert_eq!(expected.len(), before.len());
        assert_eq!(&before[20..24], &6_u32.to_le_bytes());
        // Independent full complement over the already source-qualified files.
        let word = 12 * 4096 + 14;
        let start = 12 * 4096
            + usize::from(u16::from_le_bytes(
                before[word..word + 2].try_into().unwrap(),
            ));
        let patch = start + 2 + 8;
        assert_eq!(&before[patch..patch + 8], &1000_i64.to_le_bytes());
        assert_eq!(&expected[patch..patch + 4], &final_value.to_le_bytes());
        assert_eq!(&before[..patch], &expected[..patch]);
        assert_eq!(&before[patch + 4..], &expected[patch + 4..]);

        let image =
            NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false)
                .unwrap();
        let editor = NativeJournalEditor::new(image, "Items", &mut budget(), &|| false).unwrap();
        assert_eq!(editor.update_profile(), UpdateProfile::ExistingInt32);
        let position = editor
            .snapshot()
            .columns()
            .iter()
            .position(|column| column.name() == "Large_Number")
            .unwrap();
        // The snapshot contains actual readable values and omits opaque columns;
        // it must not manufacture all sixteen native declaration positions.
        assert!(editor.snapshot().columns().len() < 16);
        assert!(
            editor
                .snapshot()
                .columns()
                .iter()
                .all(|column| column.name() != "Attachment_Field" && column.name() != "Calculated")
        );
        let handle = editor.snapshot().row_handle(0).unwrap();
        assert_eq!(
            editor.snapshot().resolve(handle).unwrap()[position],
            Value::Int32(1000)
        );
        let plan = editor
            .plan_updates(
                "Large_Number",
                &[CellUpdate::new(handle, Value::Int32(17))],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(plan.intent().updates()[0].value(), 17);
        assert_eq!(plan.intent().column(), "Large_Number");
        // Input artifacts are opened read-only and remain byte-exact.
        assert_eq!(fs::read(&before_path).unwrap(), before);
        assert_eq!(fs::read(&expected_path).unwrap(), expected);
    }
}
