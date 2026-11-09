use super::*;
use crate::workspace::DocumentView;
use crate::workspace::objects::{ObjectKind, ObjectTarget};
use crate::{
    Data, Database, Locale, Localizer, ParameterSpec, Theme,
    preview::{Preview, Scene},
};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

type Painted = Vec<(String, egui::Rect)>;

fn render(
    state: &mut State,
    size: egui::Vec2,
    events: Vec<egui::Event>,
    context: &egui::Context,
) -> (Vec<Action>, Painted) {
    let mut actions = Vec::new();
    let mut output = context.run_ui(
        egui::RawInput {
            time: Some(context.cumulative_frame_nr() as f64 * 0.25),
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ui| {
            actions = state.show(ui);
        },
    );
    let text = output
        .shapes
        .iter()
        .filter_map(|shape| {
            if let egui::epaint::Shape::Text(text) = &shape.shape {
                Some((
                    text.galley.job.text.clone(),
                    egui::Rect::from_min_size(text.pos, text.galley.size()),
                ))
            } else {
                None
            }
        })
        .collect();
    output.textures_delta.clear();
    (actions, text)
}

fn click(state: &mut State, label: &str, context: &egui::Context) -> Vec<Action> {
    let size = egui::vec2(1280.0, 820.0);
    render(state, size, vec![], context);
    let (_, text) = render(state, size, vec![], context);
    let position = text
        .iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("actual button label {label:?}"))
        .1
        .center();
    render(
        state,
        size,
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        context,
    );
    render(
        state,
        size,
        vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        context,
    )
    .0
}

#[test]
fn explicit_open_pane_has_real_open_and_picker_actions_at_supported_sizes() {
    for size in [egui::vec2(900.0, 580.0), egui::vec2(1280.0, 820.0)] {
        let context = egui::Context::default();
        let mut state = State {
            path: "sample.accdb".into(),
            file_picker_available: true,
            backstage: Backstage::Open,
            ..Default::default()
        };
        assert!(state.backstage_visible());
        if size.x == 900.0 {
            state.path = "long-directory-name/".repeat(40) + "sample.accdb";
            state.error = Some("The selected file could not be opened. ".repeat(30));
        }
        for _ in 0..3 {
            render(&mut state, size, vec![], &context);
        }
        let (actions, text) = render(&mut state, size, vec![], &context);
        assert!(actions.is_empty());
        assert!(!text.iter().any(|(text, _)| text == "All Access Objects"));
        for label in ["Database path", "Password", "Browse…", "Open database"] {
            let rect = text
                .iter()
                .find(|(text, _)| text == label)
                .expect("open control visible")
                .1;
            assert!(
                rect.left() >= 160.0 && rect.right() <= size.x,
                "control outside Open pane: {label}"
            );
            assert!(
                rect.top() > 20.0 && rect.bottom() < size.y,
                "control outside viewport: {label}"
            );
        }
    }
    let context = egui::Context::default();
    let mut state = State {
        path: "sample.accdb".into(),
        file_picker_available: true,
        backstage: Backstage::Open,
        ..Default::default()
    };
    assert!(matches!(
        click(&mut state, "Open database", &context).as_slice(),
        [Action::Open]
    ));
    assert!(matches!(
        click(&mut state, "Browse…", &context).as_slice(),
        [Action::ChooseFile]
    ));
}

#[test]
fn file_back_and_escape_preserve_document_inputs_and_running_work() {
    let context = egui::Context::default();
    let mut state = State {
        path: "original.accdb".into(),
        password: "fixture password".into(),
        sql: "SELECT 1".into(),
        ..Default::default()
    };
    state.database = Some(Database {
        title_path: None,
        document_revision: 4,
        title: "Original".into(),
        format: "ACE16".into(),
        queries: vec![],
        tables: vec![],
    });
    state.set_table_data(Data {
        row_ordinals: None,
        source_revision: Some(4),
        table: "Table".into(),
        offset: 200,
        columns: vec![],
        rows: vec![],
        has_more: true,
    });
    state.view = DocumentView::Query;
    state.dismiss_backstage();
    assert!(click(&mut state, "File", &context).is_empty());
    assert!(state.backstage_visible());
    state.busy = true;
    assert!(click(&mut state, "Back", &context).is_empty());
    assert!(!state.backstage_visible());
    state.show_open();
    let (actions, _) = render(
        &mut state,
        egui::vec2(1280.0, 820.0),
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        &context,
    );
    assert!(actions.is_empty());
    assert!(!state.backstage_visible());
    assert!(state.busy);
    assert_eq!(state.path, "original.accdb");
    assert_eq!(state.password, "fixture password");
    assert_eq!(state.sql, "SELECT 1");
    assert_eq!(state.data.as_ref().unwrap().offset, 200);
    assert!(state.view == DocumentView::Query);
}

#[test]
fn about_suppresses_hidden_query_shortcut_and_keeps_explicit_cancel_available() {
    let context = egui::Context::default();
    let mut state = State {
        sql: "SELECT 1".into(),
        ..Default::default()
    };
    state.database = Some(Database {
        title_path: None,
        document_revision: 1,
        title: "Source".into(),
        format: "ACE16".into(),
        queries: vec![],
        tables: vec![],
    });
    state.view = DocumentView::Query;
    assert!(click(&mut state, "About", &context).is_empty());
    let command = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    let (actions, _) = render(
        &mut state,
        egui::vec2(1280.0, 820.0),
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: command,
        }],
        &context,
    );
    assert!(actions.is_empty());
    state.busy = true;
    assert!(matches!(
        click(&mut state, "Cancel", &context).as_slice(),
        [Action::Cancel]
    ));
}

#[test]
fn options_changes_the_actual_theme_and_language_without_database_actions() {
    let context = egui::Context::default();
    crate::configure(&context);
    let mut state = State {
        options_open: true,
        ..Default::default()
    };

    assert!(click(&mut state, "Dark", &context).is_empty());
    assert_eq!(state.theme, Theme::Dark);
    render(&mut state, egui::vec2(1280.0, 820.0), vec![], &context);
    assert_eq!(context.theme(), egui::Theme::Dark);

    assert!(click(&mut state, "English (US)", &context).is_empty());
    assert!(click(&mut state, "日本語", &context).is_empty());
    assert_eq!(state.localizer.locale(), Locale::Ja);
    let (actions, painted) = render(&mut state, egui::vec2(1280.0, 820.0), vec![], &context);
    assert!(actions.is_empty());
    assert!(painted.iter().any(|(text, _)| text == "外観"));
    assert_eq!(state.localizer.diagnostics(), Default::default());
}

#[test]
fn options_round_trip_preserves_typed_saved_inputs_results_and_sql_draft() {
    let mut preview = Preview::new(Scene::SavedQuery);
    let state = preview.state_mut();
    let target = state.selected_saved_target().unwrap().clone();
    let saved_tab = state.active_object().unwrap();
    state.begin_saved_query_inspection(target.clone(), 80);
    assert!(state.set_saved_query_details(
        80,
        target.clone(),
        vec![
            ParameterSpec {
                name: "Minimum".into(),
                kind: "int32".into(),
                label: "LONG".into(),
                nullable: false,
            },
            ParameterSpec {
                name: "Literal choice".into(),
                kind: "text".into(),
                label: "TEXT".into(),
                nullable: true,
            },
        ]
    ));
    let sql = "SELECT '独立した SQL draft; <literal> & [name]'";
    state.sql = sql.into();
    let scratch = state
        .prepare_object_open(ObjectTarget {
            document_revision: target.document_revision,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    state.set_query_data(
        None,
        Data {
            row_ordinals: None,
            source_revision: None,
            table: "Independent SQL result".into(),
            offset: 0,
            columns: vec!["Value".into()],
            rows: vec![vec!["Earlier SQL value".into()]],
            has_more: false,
        },
    );
    let sql_rows = state.query_data.as_ref().unwrap().rows.as_ptr();
    assert!(state.activate_object(&saved_tab));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
    crate::configure(&harness.ctx);
    harness.run();
    let run = harness.state().state().localizer.text("saved-query-run");
    harness.get_by_role_and_label(Role::Button, &run).click();
    harness.run();
    for (name, value) in [
        ("Minimum", "7"),
        ("Literal choice", "NULL; <literal> & [name]"),
    ] {
        assert!(harness.state().state().saved_query_prompt_open());
        harness.get_by_role_and_label(Role::TextInput, name).focus();
        harness.run();
        harness.event(egui::Event::Text(value.into()));
        harness.run();
        let ok = harness
            .state()
            .state()
            .localizer
            .text("saved-query-prompt-ok");
        harness.get_by_role_and_label(Role::Button, &ok).click();
        harness.run();
    }
    assert!(!harness.state().state().saved_query_prompt_open());
    let Action::RunSavedQuery {
        target: original_target,
        parameters: original_inputs,
    } = harness.state().state().saved_query_run_action().unwrap()
    else {
        panic!("saved query remains ready after its synthetic result");
    };
    assert_eq!(original_inputs[0].text, "7");
    assert_eq!(original_inputs[0].kind, "int32");
    assert_eq!(original_inputs[1].text, "NULL; <literal> & [name]");
    assert!(!original_inputs[1].is_null);
    let saved_rows = harness
        .state()
        .state()
        .saved_query
        .data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    harness.get_by_role_and_label(Role::Button, &run).click();
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, "Minimum")
        .focus();
    harness.run();
    harness.event(egui::Event::Text("999".into()));
    harness.run();
    let cancel = harness
        .state()
        .state()
        .localizer
        .text("saved-query-prompt-cancel");
    harness.get_by_role_and_label(Role::Button, &cancel).click();
    harness.run();
    assert!(!harness.state().state().saved_query_prompt_open());

    let context = harness.ctx.clone();
    assert!(click(harness.state_mut().state_mut(), "File", &context).is_empty());
    assert!(click(harness.state_mut().state_mut(), "Options", &context).is_empty());
    assert!(click(harness.state_mut().state_mut(), "Dark", &context).is_empty());
    assert!(click(harness.state_mut().state_mut(), "English (US)", &context).is_empty());
    assert!(click(harness.state_mut().state_mut(), "日本語", &context).is_empty());
    let close = harness.state().state().localizer.text("backstage-close");
    assert!(click(harness.state_mut().state_mut(), &close, &context).is_empty());
    assert!(!harness.state().state().options_dialog_open());
    harness.run();

    let state = harness.state().state();
    assert!(!state.backstage_visible());
    assert_eq!(state.theme, Theme::Dark);
    assert_eq!(context.theme(), egui::Theme::Dark);
    assert_eq!(state.localizer.locale(), Locale::Ja);
    assert_eq!(state.sql, sql);
    assert_eq!(
        state.saved_query.data.as_ref().unwrap().rows.as_ptr(),
        saved_rows
    );
    assert!(
        state.query_data.is_none(),
        "inactive scratch result remains parked"
    );
    let Some(Action::RunSavedQuery { target, parameters }) = state.saved_query_run_action() else {
        panic!("saved query retains exact typed retry choices after Options");
    };
    assert_eq!(target, original_target);
    assert_eq!(parameters, original_inputs);
    assert_eq!(state.localizer.diagnostics(), Default::default());
    let saved_run = state.localizer.text("saved-query-run");
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, &saved_run)
            .accesskit_node()
            .is_disabled()
    );
    harness
        .get_by_role_and_label(Role::Button, &saved_run)
        .click();
    harness.run();
    assert!(harness.state().state().saved_query_prompt_open());
    let title = harness
        .state()
        .state()
        .localizer
        .text("saved-query-prompt-title");
    assert!(harness.query_by_label(&title).is_some());
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, "Minimum")
            .accesskit_node()
            .value()
            .as_deref()
            == Some("7")
    );
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(!harness.state().state().saved_query_prompt_open());
    let state = harness.state_mut().state_mut();
    assert!(state.activate_object(&scratch));
    assert_eq!(state.query_data.as_ref().unwrap().rows.as_ptr(), sql_rows);
    assert_eq!(state.sql, sql);
    assert_eq!(state.localizer.locale(), Locale::Ja);
    assert!(state.activate_object(&saved_tab));
    assert_eq!(
        state.saved_query.data.as_ref().unwrap().rows.as_ptr(),
        saved_rows
    );
    let Some(Action::RunSavedQuery { target, parameters }) = state.saved_query_run_action() else {
        panic!("saved input ownership survives actual object activation");
    };
    assert_eq!(target, original_target);
    assert_eq!(parameters, original_inputs);
    assert_eq!(
        harness.state().state().localizer.diagnostics(),
        Default::default()
    );
}

#[test]
fn compact_open_failure_can_copy_literal_details_without_losing_credentials_or_actions() {
    let report = "Literal open failure: <file> & [field]\n".repeat(60)
        + "FINAL OPEN DIAGNOSIS: preserve tabs\tand NULL";
    let path = "synthetic directory/Example.accdb";
    let password = "synthetic secret";
    let state = State {
        path: path.into(),
        password: password.into(),
        error: Some(report.clone()),
        backstage: Backstage::Open,
        ..Default::default()
    };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .with_max_steps(20)
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();

    for label in ["Copy details", "Open database"] {
        let button = harness.get_by_role_and_label(Role::Button, label);
        assert!(button.rect().top() >= 0.0 && button.rect().bottom() <= 580.0);
        assert!(!button.accesskit_node().is_disabled());
    }
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
    assert!(
        harness.state().1.is_empty(),
        "copying diagnostics dispatches no database action"
    );
    assert_eq!(harness.state().0.path, path);
    assert_eq!(harness.state().0.password, password);
    assert_eq!(harness.state().0.error.as_deref(), Some(report.as_str()));

    harness
        .get_by_role_and_label(Role::Button, "Open database")
        .click_accesskit();
    harness.step();
    assert!(matches!(harness.state().1.as_slice(), [Action::Open]));
    assert_eq!(harness.state().0.path, path);
    assert_eq!(harness.state().0.password, password);
    harness.state_mut().1.clear();
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.step();
    assert_eq!(harness.state().0.backstage, Backstage::Home);
    assert!(harness.state().1.is_empty());
    assert_eq!(harness.state().0.path, path);
    assert_eq!(harness.state().0.password, password);
}

#[test]
fn startup_has_no_empty_workspace_return_and_file_pages_return_to_home() {
    let state = State {
        mock_preview: false,
        path: "unchanged.accdb".into(),
        password: "unchanged secret".into(),
        ..State::default()
    };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    assert_eq!(harness.state().0.backstage, Backstage::Home);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Back")
            .is_none()
    );
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Blank database")
            .accesskit_node()
            .is_disabled()
    );
    let template_search =
        harness.get_by_role_and_label(Role::TextInput, "Search templates (unavailable)");
    assert!(template_search.accesskit_node().is_disabled());
    template_search.focus();
    harness
        .get_by_role_and_label(Role::Button, "Blank database")
        .click_accesskit();
    harness.step();
    assert_eq!(harness.state().0.backstage, Backstage::Home);
    assert!(harness.state().1.is_empty());

    harness.key_press(egui::Key::Escape);
    harness.step();
    assert_eq!(harness.state().0.backstage, Backstage::Home);

    harness
        .get_by_role_and_label(Role::Button, "Open Other Files")
        .click();
    harness.run_steps(2);
    assert_eq!(harness.state().0.backstage, Backstage::Open);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Back")
            .is_none()
    );
    for label in ["Info", "Save", "Save As", "Print", "Close", "Account"] {
        let command = harness.get_by_role_and_label(Role::Button, label);
        let node = command.accesskit_node();
        assert!(node.is_disabled());
        assert!(!node.data().supports_action(egui::accesskit::Action::Click));
        assert!(!node.data().supports_action(egui::accesskit::Action::Focus));
        command.focus();
        command.click_accesskit();
        harness.step();
        assert_eq!(harness.state().0.backstage, Backstage::Open);
        assert!(harness.state().1.is_empty());
    }
    harness.get_by_role_and_label(Role::Tab, "New").click();
    harness.run_steps(2);
    assert_eq!(harness.state().0.backstage, Backstage::New);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Blank database")
            .accesskit_node()
            .is_disabled()
    );
    harness.get_by_role_and_label(Role::Tab, "Home").click();
    harness.run_steps(2);
    assert_eq!(harness.state().0.backstage, Backstage::Home);

    harness
        .get_by_role_and_label(Role::Button, "Options")
        .click();
    harness.run_steps(2);
    assert!(harness.state().0.options_dialog_open());
    harness.key_press(egui::Key::Escape);
    harness.step();
    assert!(!harness.state().0.options_dialog_open());
    assert_eq!(harness.state().0.backstage, Backstage::Home);

    harness.get_by_role_and_label(Role::Button, "About").click();
    harness.run_steps(2);
    assert_eq!(harness.state().0.backstage, Backstage::About);
    harness.get_by_role_and_label(Role::Tab, "Home").click();
    harness.run_steps(2);
    assert_eq!(harness.state().0.backstage, Backstage::Home);
    assert!(harness.state().0.database.is_none());
    assert_eq!(harness.state().0.path, "unchanged.accdb");
    assert_eq!(harness.state().0.password, "unchanged secret");
    assert!(harness.state().1.is_empty());
}

#[test]
fn translated_file_rail_labels_fit_without_elision_in_both_themes() {
    for theme in [Theme::Light, Theme::Dark] {
        let mut preview = Preview::new(Scene::Backstage);
        let mut state = std::mem::take(preview.state_mut());
        state.mock_preview = false;
        state.localizer = Localizer::new(Locale::De).unwrap();
        state.theme = theme;
        let original_rows = state.data.as_ref().unwrap().rows.as_ptr();
        let mut harness = Harness::builder()
            .with_size(egui::vec2(900.0, 580.0))
            .build_ui_state(
                |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                (state, Vec::new()),
            );
        crate::configure(&harness.ctx);
        harness.run_steps(3);

        for label in ["Speichern unter", "Optionen"] {
            let (clip, text) = harness
                .output()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => {
                        Some((shape.clip_rect, text))
                    }
                    _ => None,
                })
                .expect("actual translated rail label");
            let font = text.galley.job.sections[0].format.font_id.clone();
            let full_width = harness.ctx.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(label.to_owned(), font.clone(), egui::Color32::WHITE)
                    .size()
                    .x
            });
            assert!(
                !text.galley.elided,
                "command {label:?} must remain visible; clip {clip:?}, font {font:?}, full width {full_width}, wrapping {:?}",
                text.galley.job.wrap
            );
            let painted = text.galley.rect.translate(text.pos.to_vec2());
            assert!(painted.left() >= clip.left());
            assert!(painted.right() <= clip.right() + 0.5);
        }
        assert!(harness.state().1.is_empty());
        assert_eq!(
            harness.state().0.data.as_ref().unwrap().rows.as_ptr(),
            original_rows
        );
    }
}

#[test]
fn localized_options_dropdowns_expose_their_setting_labels_without_changing_values() {
    for locale in [Locale::De, Locale::Ja] {
        let mut preview = Preview::new(Scene::Options);
        let mut state = std::mem::take(preview.state_mut());
        state.localizer = Localizer::new(locale).unwrap();
        state.formatting.system_locale = Some("en-US".into());
        let formatting = state.formatting.clone();
        let localizer = state.localizer.clone();
        let original_rows = state.data.as_ref().unwrap().rows.as_ptr();
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1000.0, 820.0))
            .build_ui_state(
                |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                (state, Vec::new()),
            );
        crate::configure(&harness.ctx);
        harness.run();

        for key in [
            "settings-language",
            "settings-date-format",
            "settings-time-format",
            "settings-currency-format",
        ] {
            let label = localizer.text(key);
            let control = harness.get_by_role_and_label(Role::ComboBox, label.as_str());
            assert!(!control.accesskit_node().is_disabled());
            assert!(control.accesskit_node().labelled_by().next().is_some());
        }
        assert_eq!(harness.state().0.localizer.locale(), locale);
        assert_eq!(harness.state().0.formatting, formatting);
        assert_eq!(
            harness.state().0.data.as_ref().unwrap().rows.as_ptr(),
            original_rows
        );
        assert!(harness.state().1.is_empty());
        assert_eq!(
            harness.state().0.localizer.diagnostics(),
            Default::default()
        );
    }
}

#[test]
fn about_exposes_the_exact_bundled_emoji_license_in_every_locale_without_actions() {
    for locale in Locale::ALL {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut state = State {
            backstage: Backstage::About,
            localizer: Localizer::new(locale).unwrap(),
            ..State::default()
        };
        let source = state.localizer.text("backstage-emoji-source");
        let license = state.localizer.text("backstage-emoji-license");
        let size = egui::vec2(1280.0, 820.0);
        let (actions, _) = render(&mut state, size, Vec::new(), &context);
        assert!(actions.is_empty());
        let (actions, text) = render(&mut state, size, Vec::new(), &context);
        assert!(actions.is_empty());
        assert!(text.iter().any(|(text, _)| text == &source));
        assert!(click(&mut state, &license, &context).is_empty());
        let (actions, text) = render(&mut state, size, Vec::new(), &context);
        assert!(actions.is_empty());
        assert!(
            text.iter()
                .any(|(text, _)| text.contains("Copyright 2013 Google LLC")
                    && text.contains("SIL OPEN FONT LICENSE"))
        );
    }
}
