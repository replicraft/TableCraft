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

#[test]
fn native_regional_display_survives_sorting_and_policy_changes_without_parsing_text() {
    use tablecraft_testkit::fixtures::access::{field_presentation, wide_properties};
    use tablecraft_ui::formatting::{CurrencyChoice, DateFormat, TimeFormat};

    for (kind, values, initial, raw) in [
        (
            5,
            [12_345_678, (-1_i64) as u64],
            ["€ 1.234,57", "-€ 0,00"],
            ["1234.5678", "-0.0001"],
        ),
        (
            8,
            [46124.0_f64.to_bits(), (-0.5_f64).to_bits()],
            ["12.04.2026", "12:00:00"],
            ["DATE(0x40e6858000000000)", "DATE(0xbfe0000000000000)"],
        ),
    ] {
        let blocks = if kind == 5 {
            vec![
                wide_properties::names(&["Format"]),
                field_presentation::score_group(
                    1,
                    &[field_presentation::property(
                        0,
                        1,
                        10,
                        &wide_properties::text("€ #,##0.00;-€ #,##0.00"),
                    )],
                ),
            ]
        } else {
            // No Format declaration, matching Access's General Date default.
            vec![]
        };
        let original = field_presentation::score64(&blocks, kind, &values);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("regional.mdb");
        std::fs::write(&path, &original).unwrap();
        let context = egui::Context::default();
        let mut desktop = Desktop::new(context.clone(), Some(path.clone()));
        desktop.state.formatting.system_locale = Some("de-DE".into());
        finish(&mut desktop, &context);

        let data = desktop.state.data.as_ref().unwrap();
        for (row, expected) in data.rows.iter().zip(initial) {
            assert_eq!(row[1].text, expected);
        }
        let revision = desktop.state.database.as_ref().unwrap().document_revision;
        assert!(desktop.state.can_sort_table(revision, "Items", "Id"));
        desktop.submit(Command::SortTable {
            document_revision: revision,
            table: "Items".into(),
            column: "Id".into(),
            descending: true,
        });
        finish(&mut desktop, &context);
        let sorted = desktop.state.query_data.as_ref().unwrap();
        assert_eq!(sorted.source_revision, None);
        assert!(!sorted.has_more);
        assert_eq!(sorted.rows.len(), 2);
        assert_eq!(sorted.rows[0][0].text, "2");
        assert_eq!(sorted.rows[0][1].text, initial[1]);
        assert_eq!(sorted.rows[1][1].text, initial[0]);

        desktop.state.formatting.currency = CurrencyChoice::Raw;
        desktop.state.formatting.date = DateFormat::Raw;
        desktop.state.formatting.time = TimeFormat::Raw;
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            desktop.state.show(ui);
        });
        output.textures_delta.clear();
        let sorted = desktop.state.query_data.as_ref().unwrap();
        assert_eq!(sorted.rows[0][1].text, raw[1]);
        assert_eq!(sorted.rows[1][1].text, raw[0]);

        desktop.state.formatting.currency = CurrencyChoice::SystemLocale;
        desktop.state.formatting.date = DateFormat::SystemLocale;
        desktop.state.formatting.time = TimeFormat::SystemLocale;
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            desktop.state.show(ui);
        });
        output.textures_delta.clear();
        assert_eq!(
            desktop.state.query_data.as_ref().unwrap().rows[0][1].text,
            initial[1]
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}
