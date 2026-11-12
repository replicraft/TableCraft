//! UNIQUE warning uses real localized widgets without native save authority.
use super::super::{Preview, Scene, fixtures};
use crate::{FluentArgs, Locale, Localizer, Theme};
use egui::accesskit::{Action, Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

#[test]
fn unique_index_review_has_truthful_confirmation_and_normal_disabled_ribbon() {
    for locale in [Locale::EnUs, Locale::De] {
        for theme in [Theme::Light, Theme::Dark] {
            let mut preview = Preview::new(Scene::Datasheet);
            preview.state_mut().localizer = Localizer::new(locale).unwrap();
            preview.state_mut().theme = theme;
            preview.set_scene(Scene::QuerySchemaUniqueIndexReview);
            preview.state_mut().mock_preview = false;
            let mut harness = Harness::builder()
                .with_size(egui::vec2(1280.0, 820.0))
                .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
            crate::configure(&harness.ctx);
            harness.run_steps(3);

            for (key, value) in [
                ("query-action-schema-index-name", "ScoreUnique"),
                ("query-action-schema-index-field", "CopiedScore"),
            ] {
                let mut arguments = FluentArgs::new();
                arguments.set("name", value);
                let label = harness.state().state().localizer.format(key, &arguments);
                harness.get_by_role_and_label(Role::Label, &label);
            }
            for key in [
                "query-action-schema-index-descending",
                "query-action-schema-index-unique",
                "query-action-schema-index-unique-confirmation",
            ] {
                let label = harness.state().state().localizer.text(key);
                harness.get_by_role_and_label(Role::Label, &label);
            }
            let ordinary = harness
                .state()
                .state()
                .localizer
                .text("query-action-schema-index-confirmation");
            assert!(
                harness
                    .query_by_role_and_label(Role::Label, &ordinary)
                    .is_none()
            );
            for key in ["query-action-yes", "query-action-no"] {
                let label = harness.state().state().localizer.text(key);
                let button = harness.get_by_role_and_label(Role::Button, &label);
                assert!(!button.accesskit_node().is_disabled());
            }

            let cut = harness.state().state().localizer.text("ribbon-cut");
            let unavailable = harness.state().state().localizer.text("ribbon-unavailable");
            let cut = harness.get_by_role_and_label(Role::Button, &cut);
            assert!(cut.accesskit_node().is_disabled());
            assert_eq!(
                cut.accesskit_node().description(),
                Some(unavailable.as_str())
            );
            assert!(!cut.accesskit_node().supports_action(Action::Click));

            let mut arguments = FluentArgs::new();
            arguments.set("count", fixtures::TOTAL_ROWS.to_string());
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
            assert_eq!(
                harness.state().state().sql,
                "CREATE UNIQUE INDEX [ScoreUnique] ON [Archive] ([CopiedScore] DESC)"
            );
            assert!(!harness.state().state().mock_preview);
        }
    }
}
