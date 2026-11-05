use super::*;
use egui::{Event, MouseWheelUnit, TouchPhase, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn report_scrolls_copies_and_resets_only_when_the_literal_report_changes() {
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

    // Appearance and interface language do not replace the literal diagnosis.
    crate::theme::set_theme(&harness.ctx, crate::Theme::Dark);
    let localizer = crate::Localizer::new(crate::Locale::De).unwrap();
    crate::localization::install(&harness.ctx, &localizer);
    harness.run();
    let (same, clip) = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == report => {
                Some((text, shape.clip_rect))
            }
            _ => None,
        })
        .expect("the same report remains available after appearance changes");
    let last_line = same
        .galley
        .rows
        .last()
        .unwrap()
        .rect()
        .translate(same.pos.to_vec2());
    assert!(
        clip.contains_rect(last_line),
        "same-report scroll position is retained"
    );

    // The source/widget is unchanged, and both payloads have identical lengths
    // and line geometry. A new report must not inherit the previous scroll.
    let replacement = report.replace("Literal", "Updated");
    assert_eq!(replacement.len(), report.len());
    *harness.state_mut() = replacement.clone();
    harness.run();
    let (changed, clip) = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == replacement => {
                Some((text, shape.clip_rect))
            }
            _ => None,
        })
        .expect("the complete replacement report is rendered");
    let first_line = changed
        .galley
        .rows
        .first()
        .unwrap()
        .rect()
        .translate(changed.pos.to_vec2());
    let last_line = changed
        .galley
        .rows
        .last()
        .unwrap()
        .rect()
        .translate(changed.pos.to_vec2());
    assert!(
        clip.contains_rect(first_line),
        "new diagnosis opens at its beginning"
    );
    assert!(!clip.contains_rect(last_line));
    harness
        .get_by_role_and_label(Role::Button, &localizer.text("diagnostics-copy"))
        .click_accesskit();
    harness.step();
    assert!(
        harness
            .output()
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText(replacement.clone()))
    );
    assert_eq!(harness.state(), &replacement);
}
