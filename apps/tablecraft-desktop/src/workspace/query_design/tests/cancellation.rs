//! Cancellation revokes a real delayed worker reply only for its matching Design owner.
use super::{design, opened, run, scratch, source, sql};
use crate::workspace::{Desktop, PendingOperation, test_support::finish_reads};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;
use tablecraft_runtime::{Command, Worker};

const WAIT: Duration = Duration::from_secs(10);

// Hold the actual actor at its repaint notification after it has retained each
// real reply. The UI can consume intermediate receipts and issue the next stage;
// the final reply remains buffered until cancellation or normal UI publication.
fn delayed_query_reply(
    harness: &mut Harness<'_, Desktop>,
    observed: &mpsc::Receiver<()>,
    release: &mpsc::SyncSender<()>,
) -> u64 {
    for _ in 0..8 {
        observed
            .recv_timeout(WAIT)
            .expect("actual worker reply reached its gate");
        if let PendingOperation::Query { request_id } = harness.state().pending {
            return request_id;
        }
        harness.run_steps(1);
        release.send(()).unwrap();
    }
    panic!("the real Build/Inspect/Discover workflow did not issue its Query");
}

#[test]
fn matching_design_cancel_revokes_delayed_real_reply_and_other_ids_cannot_cancel_sql() {
    let (_directory, path, original, mut harness) = opened(source());
    let armed = Arc::new(AtomicBool::new(false));
    // NO_CLONE: The test controls the actor gate while its repaint closure owns
    // an independent shared handle; no query/source/result payload is copied.
    let actor_gate = Arc::clone(&armed);
    let (entered, observed) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let released = Mutex::new(released);
    harness.state_mut().worker = Worker::new(move || {
        if actor_gate.load(Ordering::Acquire) {
            entered.send(()).unwrap();
            released.lock().unwrap().recv_timeout(WAIT).unwrap();
        }
    });
    // NO_CLONE: Opening transfers a path into the new real worker; the test
    // independently retains its path to verify the database after cancellation.
    harness.state_mut().open_request(Command::Open {
        path: path.clone(),
        password: String::new(),
    });
    finish_reads(&mut harness);
    scratch(&mut harness);
    sql(&mut harness, "SELECT 99 AS Previous");
    run(&mut harness);
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    sql(
        &mut harness,
        "SELECT Score,Sum(Id) AS Total FROM Items GROUP BY Score",
    );
    design(&mut harness);
    armed.store(true, Ordering::Release);
    harness
        .get_all_by_role_and_label(Role::Button, "Run query")
        .next()
        .unwrap()
        .click_accesskit();
    harness.run_steps(1);
    let run_id = delayed_query_reply(&mut harness, &observed, &release);
    assert_eq!(
        harness.state().state.query_design_request_id(),
        Some(run_id)
    );
    assert!(harness.state().state.busy);

    harness.state_mut().cancel_query_design(run_id + 1);
    assert_eq!(
        harness.state().state.query_design_request_id(),
        Some(run_id)
    );
    assert!(harness.state().state.busy);
    harness.state_mut().cancel_query_design(run_id);
    assert_eq!(harness.state().state.query_design_request_id(), None);
    assert!(!harness.state().state.busy);
    assert!(
        harness.state().worker.poll().is_none(),
        "actual worker reply was revoked"
    );
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        previous
    );
    assert!(harness.state().state.query_design_draft().unwrap().totals);
    armed.store(false, Ordering::Release);
    release.send(()).unwrap();
    harness.run_steps(2);
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        previous
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);

    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "SQL View")
        .click_accesskit();
    harness.run_steps(2);
    sql(&mut harness, "SELECT 7 AS Ordinary");
    armed.store(true, Ordering::Release);
    harness
        .get_all_by_role_and_label(Role::Button, "Run query")
        .next()
        .unwrap()
        .click_accesskit();
    harness.run_steps(1);
    let ordinary_id = delayed_query_reply(&mut harness, &observed, &release);
    assert_eq!(harness.state().state.query_design_request_id(), None);
    harness.state_mut().cancel_query_design(ordinary_id);
    assert!(
        harness.state().state.busy,
        "ordinary SQL owner was not cancelled"
    );
    armed.store(false, Ordering::Release);
    release.send(()).unwrap();
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "7"
    );
    assert!(!harness.state().editing.has_save());
    assert!(!tablecraft_runtime::journal_path(&path).exists());
    assert_eq!(std::fs::read(path).unwrap(), original);
}
