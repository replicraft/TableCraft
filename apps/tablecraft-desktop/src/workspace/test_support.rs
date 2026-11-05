//! Application-private real host glue shared by headless workflow acceptance tests.
use super::Desktop;
use eframe::App;
use egui_kittest::Harness;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub(super) fn opened(
    original: Vec<u8>,
) -> (
    tempfile::TempDir,
    PathBuf,
    Vec<u8>,
    Harness<'static, Desktop>,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mutation.accdb");
    std::fs::write(&path, &original).unwrap();
    let desktop = Desktop::new(egui::Context::default(), Some(path.clone()));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 760.0))
        .with_max_steps(32)
        .build_ui_state(
            |ui, desktop: &mut Desktop| {
                let mut frame = eframe::Frame::_new_kittest();
                desktop.logic(ui.ctx(), &mut frame);
                desktop.ui(ui, &mut frame);
            },
            desktop,
        );
    tablecraft_ui::configure(&harness.ctx);
    finish_reads(&mut harness);
    (directory, path, original, harness)
}

pub(super) fn finish_reads(harness: &mut Harness<'_, Desktop>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy {
        assert!(Instant::now() < deadline, "native host read did not finish");
        harness.run_steps(1);
        std::thread::yield_now();
    }
    harness.run_steps(2);
    assert!(harness.state().state.error.is_none());
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(super) fn finish_save(harness: &mut Harness<'_, Desktop>) -> tablecraft_runtime::SaveReport {
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().editing.save_report().is_none() {
        assert!(Instant::now() < deadline, "native host save did not finish");
        harness.run_steps(1);
        std::thread::yield_now();
    }
    harness.state().editing.save_report().unwrap()
}
