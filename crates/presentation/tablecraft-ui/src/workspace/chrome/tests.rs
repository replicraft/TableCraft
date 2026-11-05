use super::*;
use crate::Database;

#[test]
fn long_workspace_title_retains_full_identity_without_displacing_status() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let title = "Quarterly international customer and supplier reconciliation workspace ".repeat(8);
    let mut state = State {
        database: Some(Database {
            document_revision: 7,
            title: title.clone(),
            format: "ACE16".into(),
            tables: vec![],
            queries: vec![],
        }),
        ..Default::default()
    };
    let status = state.edit_status();
    for _ in 0..3 {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 580.0),
                )),
                ..Default::default()
            },
            |ui| state.chrome(ui, &mut Vec::new()),
        );
        output.textures_delta.clear();
        let title_text = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.job.text == title => {
                    Some((text, shape.clip_rect))
                }
                _ => None,
            })
            .expect("workspace title is painted");
        let status_text = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text)
                    if text.galley.job.text == status && text.pos.y < 60.0 =>
                {
                    Some((text, shape.clip_rect))
                }
                _ => None,
            })
            .expect("title-bar status is painted");
        let title_rect = egui::Rect::from_min_size(title_text.0.pos, title_text.0.galley.size());
        let status_rect = egui::Rect::from_min_size(status_text.0.pos, status_text.0.galley.size());
        assert!(
            title_text.0.galley.elided,
            "long title stays on one truncated line"
        );
        assert!(title_text.1.contains_rect(title_rect));
        assert!(status_text.1.contains_rect(status_rect));
        assert!(status_rect.right() <= 900.0);
        assert!(title_rect.right() <= status_rect.left());
        let tree = output.platform_output.accesskit_update.as_ref().unwrap();
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| node.label() == Some(title.as_str())
                    || node.value() == Some(title.as_str()))
        );
    }
}

#[test]
fn short_workspace_title_keeps_the_same_left_edge_as_a_truncated_title() {
    let context = egui::Context::default();
    let mut state = State::default();
    let mut starts = Vec::new();
    for title in ["Long workspace identity ".repeat(40), "Sample.accdb".into()] {
        state.database = Some(Database {
            document_revision: 7,
            title: title.clone(),
            format: "ACE16".into(),
            tables: vec![],
            queries: vec![],
        });
        for frame in 0..3 {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900.0, 580.0),
                    )),
                    ..Default::default()
                },
                |ui| state.chrome(ui, &mut Vec::new()),
            );
            output.textures_delta.clear();
            if frame == 2 {
                let x = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::epaint::Shape::Text(text) if text.galley.job.text == title => {
                            Some(text.pos.x)
                        }
                        _ => None,
                    })
                    .expect("workspace title is painted");
                starts.push(x);
            }
        }
    }
    assert!(
        (starts[0] - starts[1]).abs() < 0.1,
        "short titles stay beside the application name"
    );
}
