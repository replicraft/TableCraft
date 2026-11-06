use crate::{FontFamilies, Locale, Localizer};
use egui::{Event, Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

#[test]
fn search_is_first_filters_as_you_type_and_resets_on_reopen() {
    for locale in [Locale::EnUs, Locale::De] {
        let localizer = Localizer::new(locale).unwrap();
        let search_label = localizer.text("ribbon-font-search");
        let font_label = localizer.text("ribbon-font");
        let empty_label = localizer.text("ribbon-font-no-results");
        let families = FontFamilies::from_system(["Example Serif", "Verdana"]).unwrap();
        let mut harness = Harness::builder()
            .with_size(egui::vec2(650.0, 420.0))
            .build_ui(move |ui| {
                crate::localization::install(ui.ctx(), &localizer);
                super::super::groups::home(ui, &localizer, families.families());
            });
        crate::configure(&harness.ctx);
        harness.run_steps(3);
        harness
            .get_by_role_and_label(Role::ComboBox, &font_label)
            .click_accesskit();
        harness.run_steps(3);
        let search = harness.get_by_role_and_label(Role::TextInput, &search_label);
        let first = harness.get_by_role_and_label(Role::Button, "Example Serif");
        assert!(search.rect().bottom() <= first.rect().top());
        assert!(search.accesskit_node().is_focused());
        search.click();
        harness.run_steps(3);

        harness.event(Event::Text("sElA".into()));
        harness.run_steps(3);
        harness.get_by_role_and_label(Role::Button, "Selawik");
        assert!(
            harness
                .query_by_role_and_label(Role::Button, "Example Serif")
                .is_none()
        );
        assert!(
            harness
                .query_by_role_and_label(Role::Button, "Verdana")
                .is_none()
        );
        assert_eq!(
            harness
                .get_by_role_and_label(Role::TextInput, &search_label)
                .accesskit_node()
                .value()
                .as_deref(),
            Some("sElA")
        );

        harness.event(Event::Text("no-match".into()));
        harness.run_steps(3);
        harness.get_by_role_and_label(Role::Label, &empty_label);
        assert!(
            harness
                .query_by_role_and_label(Role::Button, "Selawik")
                .is_none()
        );
        harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
        harness.key_press(Key::Backspace);
        harness.run_steps(3);
        harness.get_by_role_and_label(Role::Button, "Example Serif");
        assert!(harness.output().platform_output.commands.is_empty());

        harness.event(Event::Text("Verdana".into()));
        harness.run_steps(3);
        harness.get_by_role_and_label(Role::Button, "Verdana");
        harness.key_press(Key::Escape);
        harness.run_steps(3);
        assert!(
            harness
                .query_by_role_and_label(Role::TextInput, &search_label)
                .is_none()
        );
        harness
            .get_by_role_and_label(Role::ComboBox, &font_label)
            .click_accesskit();
        harness.run_steps(3);
        assert_eq!(
            harness
                .get_by_role_and_label(Role::TextInput, &search_label)
                .accesskit_node()
                .value()
                .as_deref(),
            Some("")
        );
        harness.get_by_role_and_label(Role::Button, "Example Serif");
        assert_eq!(
            harness
                .get_by_role_and_label(Role::ComboBox, &font_label)
                .accesskit_node()
                .value()
                .as_deref(),
            Some("Selawik")
        );
    }
}
