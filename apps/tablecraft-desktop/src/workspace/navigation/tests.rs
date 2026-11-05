//! Host requests use the real immutable native page service.
use super::*;
use eframe::App;
use std::time::{Duration, Instant};
use tablecraft_testkit::fixtures::access::mutation_fixture;

fn finish(desktop: &mut Desktop, context: &egui::Context) {
    let mut frame = eframe::Frame::_new_kittest();
    let deadline = Instant::now() + Duration::from_secs(10);
    while desktop.state.busy {
        assert!(Instant::now() < deadline, "native page did not finish");
        desktop.logic(context, &mut frame);
        std::thread::yield_now();
    }
    assert!(desktop.state.error.is_none());
}

fn opened() -> (tempfile::TempDir, egui::Context, Desktop) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("records.mdb");
    let records = (1..=105).map(|id| (id, id * 10)).collect::<Vec<_>>();
    std::fs::write(&path, mutation_fixture::with_rows(&records)).unwrap();
    let context = egui::Context::default();
    let mut desktop = Desktop::new(context.clone(), Some(path));
    finish(&mut desktop, &context);
    assert_eq!(desktop.state.data.as_ref().unwrap().table, "Items");
    assert_eq!(desktop.state.data.as_ref().unwrap().rows.len(), 100);
    (directory, context, desktop)
}

fn target(desktop: &Desktop, ordinal: u32) -> RecordTarget {
    RecordTarget {
        document_revision: desktop.state.database.as_ref().unwrap().document_revision,
        table: "Items".into(),
        parent_ordinal: ordinal,
        column: "Score".into(),
    }
}

#[test]
fn global_record_reads_cross_the_page_boundary_and_retain_the_old_page_at_eof() {
    let (_directory, context, mut desktop) = opened();
    let request = target(&desktop, 100);
    desktop.navigate_record(request.clone());
    let generation = desktop.generation;
    assert!(desktop.state.busy);
    assert_eq!(desktop.state.data.as_ref().unwrap().offset, 0);

    desktop.navigate_record(target(&desktop, 101));
    assert_eq!(
        desktop.generation, generation,
        "pending read is not replaced"
    );
    finish(&mut desktop, &context);
    let data = desktop.state.data.as_ref().unwrap();
    assert_eq!(data.offset, 100);
    assert_eq!(data.rows.len(), 5);
    assert_eq!(data.rows[0][1].text, "1010");
    assert!(!data.has_more);

    desktop.navigate_record(target(&desktop, 105));
    finish(&mut desktop, &context);
    let data = desktop.state.data.as_ref().unwrap();
    assert_eq!(
        data.offset, 100,
        "checked EOF does not publish an empty replacement"
    );
    assert_eq!(data.rows.len(), 5);
    assert!(desktop.state.can_navigate_record(&request));

    desktop.navigate_record(target(&desktop, 99));
    finish(&mut desktop, &context);
    let data = desktop.state.data.as_ref().unwrap();
    assert_eq!(data.offset, 99);
    assert_eq!(data.rows[0][1].text, "1000");
}

#[test]
fn stale_and_hidden_record_requests_cannot_replace_a_new_open() {
    let (_directory, context, mut desktop) = opened();
    let stale = target(&desktop, 100);
    desktop.open();
    let open_generation = desktop.generation;
    desktop.navigate_record(stale.clone());
    assert_eq!(desktop.generation, open_generation);
    finish(&mut desktop, &context);

    desktop.submit(Command::Page {
        table: "Items".into(),
        offset: 0,
    });
    finish(&mut desktop, &context);
    let generation = desktop.generation;
    desktop.navigate_record(stale);
    assert_eq!(desktop.generation, generation);
    assert!(!desktop.state.busy);

    desktop.state.show_open();
    desktop.navigate_record(target(&desktop, 100));
    assert_eq!(desktop.generation, generation);
}

#[test]
fn automatic_first_table_and_title_belong_to_the_successfully_opened_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("TestDatabase.mdb");
    std::fs::write(&path, mutation_fixture::database()).unwrap();
    let context = egui::Context::default();
    let mut desktop = Desktop::new(context.clone(), Some(path.clone()));
    // Editable Open text must not retitle an in-flight successful capture.
    desktop.state.path = "different.accdb".into();
    finish(&mut desktop, &context);

    let database = desktop.state.database.as_ref().unwrap();
    assert_eq!(
        database.title,
        "TestDatabase : Database | TestDatabase.mdb (Access 2000–2003)"
    );
    assert_eq!(
        database.title_path.as_deref(),
        Some(path.to_string_lossy().as_ref())
    );
    let data = desktop.state.data.as_ref().unwrap();
    assert_eq!(data.table, "Items");
    assert_eq!(data.offset, 0);
    assert_eq!(data.rows.len(), 3);
    assert_eq!(data.source_revision, Some(database.document_revision));
}
