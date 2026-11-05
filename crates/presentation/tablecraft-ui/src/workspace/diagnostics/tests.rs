use super::*;
use egui::{Event, MouseWheelUnit, TouchPhase, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn report_scrolls_to_its_final_line_and_copies_the_complete_literal_payload() {
    let report = "Literal <record> & [field] \"quoted\"\n".repeat(40)
        + "FINAL DIAGNOSIS: keep tabs\tand a literal NULL";
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 300.0))
        .with_max_steps(32)
        .build_ui_state(
            |ui, report: &mut String| show(ui, Id::new("literal_report"), report),
            report.clone(),
        );
    crate::configure(&harness.ctx);
    harness.run();
    let viewport = harness.get_by_role(Role::ScrollBar).rect();
    assert!(viewport.height() <= MAX_REPORT_HEIGHT);

    harness.event(Event::PointerMoved(egui::pos2(450.0, viewport.center().y)));
    harness.event(Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -10000.0),
        modifiers: egui::Modifiers::NONE,
        phase: TouchPhase::Move,
    });
    harness.run_steps(12);
    let (text, clip) = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == report => {
                Some((text, shape.clip_rect))
            }
            _ => None,
        })
        .expect("complete literal report remains in the bounded viewport");
    let last_line = text
        .galley
        .rows
        .last()
        .unwrap()
        .rect()
        .translate(text.pos.to_vec2());
    assert!(clip.contains_rect(last_line));

    harness
        .get_by_role_and_label(Role::Button, "Copy details")
        .click_accesskit();
    harness.step();
    assert!(
        harness
            .output()
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText(report.clone()))
    );
    assert_eq!(harness.state(), &report);
}
