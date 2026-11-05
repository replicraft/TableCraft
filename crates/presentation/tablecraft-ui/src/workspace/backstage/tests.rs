use super::*;
use crate::workspace::DocumentView;
use crate::{Data, Database};

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
        .expect("actual button label")
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
fn default_open_pane_has_real_open_and_picker_actions_at_supported_sizes() {
    for size in [egui::vec2(900.0, 580.0), egui::vec2(1280.0, 820.0)] {
        let context = egui::Context::default();
        let mut state = State {
            path: "sample.accdb".into(),
            file_picker_available: true,
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
        document_revision: 4,
        title: "Original".into(),
        format: "ACE16".into(),
        queries: vec![],
        tables: vec![],
    });
    state.set_table_data(Data {
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
        backstage: Backstage::Options,
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
