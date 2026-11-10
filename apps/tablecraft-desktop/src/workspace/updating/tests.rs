use super::*;
use egui::Context;

#[test]
fn clean_host_allows_restart_but_unsaved_sql_and_pending_reads_block_install_dispatch() {
    let mut app = Desktop::new(Context::default(), None);
    app.state.sql.clear();
    assert!(app.can_install_update());

    app.state.sql = "SELECT 42".into();
    app.state.set_update_status(UpdateStatus::Ready {
        version: "0.1.1".into(),
    });
    assert!(!app.can_install_update());
    app.start_update(UpdateCommand::Install);
    assert!(matches!(
        app.state.update_status(),
        UpdateStatus::Failed(UpdateFailure::Installation)
    ));
    assert!(!app.updates.installing());

    app.state.sql.clear();
    app.state.busy = true;
    assert!(!app.can_install_update());
    app.start_update(UpdateCommand::Install);
    assert!(!app.updates.installing());
}
