use super::*;
use crate::Action;
use crate::workspace::preview::Scene;

fn rendered_text(preview: &mut Preview) -> (Vec<String>, Vec<String>) {
    let context = egui::Context::default();
    context.enable_accesskit();
    crate::configure(&context);
    let mut text = Vec::new();
    let mut accessible = Vec::new();
    for _ in 0..3 {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 820.0),
                )),
                ..Default::default()
            },
            |ui| assert!(preview.state.show(ui).is_empty()),
        );
        text = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .collect();
        accessible = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .filter_map(|(_, node)| node.value().or_else(|| node.label()).map(str::to_owned))
            .collect();
        output.textures_delta.clear();
    }
    (text, accessible)
}

fn plain(text: &str) -> String {
    text.chars()
        .filter(|character| !matches!(character, '\u{2068}' | '\u{2069}'))
        .collect()
}

#[test]
fn collection_lifecycle_scenes_preserve_the_parent_and_reject_stale_replies() {
    for (scene, expected) in [
        (CollectionScene::Loading, "Reading stored contents…"),
        (CollectionScene::Error, "Synthetic collection read error"),
        (
            CollectionScene::Unassigned,
            "No assigned multivalue collection",
        ),
        (
            CollectionScene::Empty,
            "Assigned multivalue collection; 0 complete members",
        ),
    ] {
        let mut preview = Preview::new(Scene::Datasheet);
        preview.state.path = "/synthetic/path/that/must/not/be/opened.accdb".into();
        preview.prepare_collection_scene(scene);
        let source = preview.state.data.as_ref().unwrap();
        let identity = (
            source.source_revision,
            source.table.clone(),
            source.offset,
            source.columns.clone(),
        );
        let request = preview
            .state
            .contents_request_id()
            .expect("bound synthetic collection request");
        let column = if matches!(scene, CollectionScene::Loading | CollectionScene::Error) {
            "Documents"
        } else {
            "Categories"
        };
        let target = source
            .contents_target(0, source.column_index(column).unwrap())
            .unwrap();
        let (text, _) = rendered_text(&mut preview);
        assert!(
            text.iter()
                .map(|text| plain(text))
                .any(|text| text.starts_with(expected)),
            "missing scene text: {expected}"
        );
        assert!(preview.state.selected_contents_text().is_none());
        assert!(!preview.state.can_copy_contents_selection());
        let source = preview.state.data.as_ref().unwrap();
        assert_eq!(
            (
                source.source_revision,
                source.table.clone(),
                source.offset,
                source.columns.clone()
            ),
            identity
        );
        assert_eq!(preview.state.contents_request_id(), Some(request));

        // Closing a synthetic dialog invalidates both a late success and failure.
        preview.dispatch(Action::CloseContents {
            request_id: request,
        });
        preview.state.set_contents(
            request,
            &target,
            CollectionKind::Attachments,
            true,
            fixtures::contents(CollectionKind::Attachments),
        );
        preview
            .state
            .fail_contents(request, &target, "late synthetic failure".into());
        assert!(preview.state.contents_request_id().is_none());
    }
}

#[test]
fn null_members_remain_members_and_literal_text_is_not_reinterpreted() {
    for (scene, column, count, expected_kind) in [
        (CollectionScene::Null, "Categories", 4, "multivalue"),
        (
            CollectionScene::AttachmentNull,
            "Documents",
            1,
            "attachment",
        ),
        (CollectionScene::HistoryNull, "Notes", 3, "stored version"),
    ] {
        let mut preview = Preview::new(Scene::Datasheet);
        preview.prepare_collection_scene(scene);
        let parent = preview.state.data.as_ref().unwrap();
        let cell = &parent.rows[0][parent.column_index(column).unwrap()];
        let actual_count = match cell.kind {
            CellKind::Multivalue {
                assigned: true,
                members,
            }
            | CellKind::Attachments {
                assigned: true,
                members,
            }
            | CellKind::VersionHistory {
                assigned: true,
                members,
            } => members,
            _ => panic!("collection assignment and complete membership must be retained"),
        };
        assert_eq!(actual_count, count);
        let (text, accessible) = rendered_text(&mut preview);
        let membership = format!("Assigned {expected_kind} collection; {count} complete");
        assert!(
            text.iter()
                .map(|text| plain(text))
                .any(|text| text.starts_with(&membership))
        );
        let null_column = if matches!(scene, CollectionScene::AttachmentNull) {
            "FileName"
        } else {
            "Value"
        };
        let typed_null = format!("{null_column}, row 1: NULL; NULL value");
        assert!(
            accessible
                .iter()
                .map(|text| plain(text))
                .any(|text| text.starts_with(&typed_null))
        );
        if matches!(scene, CollectionScene::Null) {
            assert!(text.iter().any(|text| text == "NULL"));
            assert_eq!(
                text.iter()
                    .filter(|text| text.as_str() == "Needs \"approval\"; follow-up")
                    .count(),
                2
            );
        }
        if matches!(scene, CollectionScene::AttachmentNull) {
            assert!(
                text.iter()
                    .any(|text| text.contains("Attachment bytes remain encoded"))
            );
            assert!(text.iter().any(|text| text == "application/octet-stream"));
        }
        if matches!(scene, CollectionScene::HistoryNull) {
            for timestamp in ["2026-05-12 09:30", "2026-04-18 14:15", "2026-03-02 11:00"] {
                assert!(text.iter().any(|text| text == timestamp));
            }
            assert!(
                text.iter()
                    .any(|text| text.contains("current Memo text is separate"))
            );
        }
        assert!(preview.state.can_copy_contents_selection());
        let selected = if matches!(scene, CollectionScene::HistoryNull) {
            "2026-05-12 09:30"
        } else {
            "NULL"
        };
        assert_eq!(preview.state.selected_contents_text(), Some(selected));
    }
}

#[test]
fn closing_and_reopening_collection_scenes_retains_membership_and_literal_payloads() {
    for scene in [
        Scene::CollectionLoading,
        Scene::CollectionError,
        Scene::CollectionUnassigned,
        Scene::CollectionEmpty,
        Scene::CollectionNull,
        Scene::AttachmentNull,
        Scene::HistoryNull,
    ] {
        let mut preview = Preview::new(scene);
        let collection_scene = collection_scene(scene).unwrap();
        let (_, column) = collection_scene.source();
        let parent = preview.state.data.as_ref().unwrap();
        let column_index = parent.column_index(column).unwrap();
        let target = parent.contents_target(0, column_index).unwrap();
        let parent_cell = (
            parent.rows[0][column_index].text.clone(),
            parent.rows[0][column_index].kind,
        );
        let selection = preview.state.table_selection.position();
        let selected_child = preview.state.selected_contents_text().map(str::to_owned);
        let (mut before_text, mut before_accessible) = rendered_text(&mut preview);
        before_text.sort();
        before_accessible.sort();
        let request_id = preview.state.contents_request_id().unwrap();
        preview.dispatch(Action::CloseContents { request_id });
        assert!(preview.state.contents_request_id().is_none());
        preview.dispatch(Action::ViewContents { target });
        assert!(preview.state.contents_request_id().is_some());
        let (mut after_text, mut after_accessible) = rendered_text(&mut preview);
        after_text.sort();
        after_accessible.sort();
        assert_eq!(
            after_text, before_text,
            "scene {scene:?} preserves displayed contents"
        );
        assert_eq!(
            after_accessible, before_accessible,
            "scene {scene:?} preserves typed NULL observations"
        );
        assert_eq!(
            preview.state.selected_contents_text(),
            selected_child.as_deref()
        );
        assert_eq!(preview.state.table_selection.position(), selection);
        let parent = preview.state.data.as_ref().unwrap();
        assert_eq!(parent.rows[0][column_index].text, parent_cell.0);
        assert!(parent.rows[0][column_index].kind == parent_cell.1);
    }
}

#[test]
fn nullable_collection_fixture_does_not_replace_another_parent_rows_members() {
    let mut preview = Preview::new(Scene::CollectionNull);
    let parent = preview.state.data.as_ref().unwrap();
    let column = parent.column_index("Categories").unwrap();
    let target = parent.contents_target(1, column).unwrap();
    let request_id = preview.state.contents_request_id().unwrap();
    preview.dispatch(Action::CloseContents { request_id });
    preview.dispatch(Action::ViewContents { target });
    let (text, accessible) = rendered_text(&mut preview);
    assert!(
        text.iter()
            .map(|text| plain(text))
            .any(|text| { text.starts_with("Assigned multivalue collection; 2 complete members") })
    );
    assert!(
        !accessible
            .iter()
            .map(|text| plain(text))
            .any(|text| { text.starts_with("Value, row 1: NULL; NULL value") })
    );
}

#[test]
fn stale_collection_close_preserves_the_newer_request() {
    let mut preview = Preview::new(Scene::Multivalue);
    let old = preview.state.contents_request_id().unwrap();
    let data = preview.state.data.as_ref().unwrap();
    let column = data.column_index("Categories").unwrap();
    let target = data.contents_target(0, column).unwrap();
    preview.collection(target, CollectionKind::Multivalue);
    let current = preview.state.contents_request_id().unwrap();
    assert_ne!(current, old);
    preview.dispatch(Action::CloseContents { request_id: old });
    assert_eq!(preview.state.contents_request_id(), Some(current));
    preview.dispatch(Action::CloseContents {
        request_id: current,
    });
    assert!(preview.state.contents_request_id().is_none());
}
