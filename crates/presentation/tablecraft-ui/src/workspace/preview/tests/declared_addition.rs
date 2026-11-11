//! Declared ADD warning facts carry no native capability or record effects.
use super::super::{Preview, Scene};
use crate::{FluentArgs, Locale, Localizer, Theme};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn declared_add_review_scene_names_exact_field_type_and_schema_only_warning() {
    for locale in [Locale::EnUs, Locale::De] {
        for theme in [Theme::Light, Theme::Dark] {
            let mut preview = Preview::new(Scene::Datasheet);
            preview.state_mut().localizer = Localizer::new(locale).unwrap();
            preview.state_mut().theme = theme;
            preview.set_scene(Scene::QuerySchemaAddColumnReview);
            let mut harness = Harness::builder()
                .with_size(egui::vec2(1280.0, 820.0))
                .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
            crate::configure(&harness.ctx);
            harness.run_steps(3);
            harness.get_by_role_and_label(Role::Label, "Amount");
            for key in [
                "design-field-name",
                "design-data-type",
                "design-type-currency",
                "query-action-schema-add-confirmation",
            ] {
                let label = harness.state().state().localizer.text(key);
                harness.get_by_role_and_label(Role::Label, &label);
            }
            for key in ["query-action-yes", "query-action-no"] {
                let label = harness.state().state().localizer.text(key);
                harness.get_by_role_and_label(Role::Button, &label);
            }
            let mut arguments = FluentArgs::new();
            arguments.set("count", "0");
            for key in ["query-action-matched", "query-action-update-changed"] {
                let label = harness.state().state().localizer.format(key, &arguments);
                assert!(
                    harness
                        .query_by_role_and_label(Role::Label, &label)
                        .is_none()
                );
            }
            assert!(harness.state().state().mock_preview);
        }
    }
}
