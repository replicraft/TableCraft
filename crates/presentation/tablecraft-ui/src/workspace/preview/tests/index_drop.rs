//! Read-only mock facts exercise the real warning without minting a native plan.
use super::super::{Preview, Scene, fixtures};
use crate::{FluentArgs, Locale, Localizer, Theme};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn index_drop_warning_has_localized_names_and_decisions_without_data_or_retirement_counts() {
    for locale in [Locale::EnUs, Locale::De] {
        for theme in [Theme::Light, Theme::Dark] {
            let mut preview = Preview::new(Scene::Datasheet);
            preview.state_mut().localizer = Localizer::new(locale).unwrap();
            preview.state_mut().theme = theme;
            preview.set_scene(Scene::QuerySchemaIndexDropReview);
            let mut harness = Harness::builder()
                .with_size(egui::vec2(1280.0, 820.0))
                .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
            crate::configure(&harness.ctx);
            harness.run_steps(3);
            let mut arguments = FluentArgs::new();
            arguments.set("name", "ScoreIndex");
            let label = harness
                .state()
                .state()
                .localizer
                .format("query-action-schema-drop-index-name", &arguments);
            harness.get_by_role_and_label(Role::Label, &label);
            for key in [
                "query-action-schema-drop-index-confirmation",
                "query-action-no-undo",
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
            for key in [
                "query-action-matched",
                "query-action-update-changed",
                "query-action-schema-index-source-rows",
            ] {
                let label = harness.state().state().localizer.format(key, &arguments);
                assert!(
                    harness
                        .query_by_role_and_label(Role::Label, &label)
                        .is_none()
                );
            }
            assert!(harness.state().state().mock_preview);
            assert_eq!(
                harness.state().state().sql,
                "DROP INDEX [ScoreIndex] ON [Archive]"
            );
        }
    }
}
