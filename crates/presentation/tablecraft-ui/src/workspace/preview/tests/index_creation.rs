//! Index warning facts are localized presentation, never a runtime capability.
use super::super::{Preview, Scene, fixtures};
use crate::{FluentArgs, Locale, Localizer, Theme};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn index_review_scene_shows_exact_metadata_and_schema_only_counts() {
    for locale in [Locale::EnUs, Locale::De] {
        for theme in [Theme::Light, Theme::Dark] {
            let mut preview = Preview::new(Scene::Datasheet);
            preview.state_mut().localizer = Localizer::new(locale).unwrap();
            preview.state_mut().theme = theme;
            preview.set_scene(Scene::QuerySchemaIndexReview);
            let mut harness = Harness::builder()
                .with_size(egui::vec2(1280.0, 820.0))
                .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
            crate::configure(&harness.ctx);
            harness.run_steps(3);
            for (key, value) in [
                ("query-action-schema-index-name", "ScoreIndex"),
                ("query-action-schema-index-field", "CopiedScore"),
            ] {
                let mut arguments = FluentArgs::new();
                arguments.set("name", value);
                let label = harness.state().state().localizer.format(key, &arguments);
                harness.get_by_role_and_label(Role::Label, &label);
            }
            for key in [
                "query-action-schema-index-descending",
                "query-action-schema-index-confirmation",
            ] {
                let label = harness.state().state().localizer.text(key);
                harness.get_by_role_and_label(Role::Label, &label);
            }
            for key in ["query-action-yes", "query-action-no"] {
                let label = harness.state().state().localizer.text(key);
                harness.get_by_role_and_label(Role::Button, &label);
            }
            let mut arguments = FluentArgs::new();
            arguments.set("count", fixtures::TOTAL_ROWS.to_string());
            let population = harness
                .state()
                .state()
                .localizer
                .format("query-action-schema-index-source-rows", &arguments);
            harness.get_by_role_and_label(Role::Label, &population);
            let matched = harness
                .state()
                .state()
                .localizer
                .format("query-action-matched", &arguments);
            assert!(
                harness
                    .query_by_role_and_label(Role::Label, &matched)
                    .is_none()
            );
            assert!(harness.state().state().mock_preview);
        }
    }
}
