use super::*;
use crate::Database;

#[test]
fn retained_artifact_notice_is_revision_bound_and_explicitly_clearable() {
    let mut state = State {
        database: Some(Database {
            title_path: None,
            document_revision: 7,
            title: "Example".into(),
            format: "Access 2016".into(),
            tables: vec![],
            queries: vec![],
        }),
        ..Default::default()
    };
    let notice = Some(SaveCleanupNotice::RetainedPrivateJournal);
    state.set_save_cleanup(6, notice);
    assert_eq!(state.save_cleanup_status(), None);

    state.set_save_cleanup(7, notice);
    assert_eq!(
        state.save_cleanup_status(),
        Some("editing-save-retained-private-journal")
    );
    state.database.as_mut().unwrap().document_revision = 8;
    assert_eq!(state.save_cleanup_status(), None);
    state.set_save_cleanup(8, notice);
    state.set_save_cleanup(8, None);
    assert_eq!(state.save_cleanup_status(), None);
}
