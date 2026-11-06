use super::super::{source_area, tests::session};
use crate::workspace::query_design::Session;
use egui::{Event, Key, Modifiers};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::query::{
    DesignColumn, DesignExpression, DesignField, DesignFieldMetadata,
};

fn harness(session: Session) -> Harness<'static, Session> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(600.0, 340.0))
        .build_ui_state(
            |ui, session| {
                source_area(ui, session, 280.0);
            },
            session,
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
}

#[test]
fn arrows_reveal_local_fields_and_keyboard_output_preserves_existing_column_semantics() {
    let mut state = session();
    state.metadata[0]
        .fields
        .extend((0..24).map(|index| DesignFieldMetadata {
            name: format!("Field {index:02}"),
            scalar_type: None,
        }));
    // Imported SQL retains its spelling even when metadata uses CustomerID.
    let mut column = DesignColumn::field(1, "customerid".into());
    column.alias = Some("Retained alias".into());
    column.criteria = vec!["> 0".into()];
    state.draft.columns.push(column.clone());
    state.draft.columns.push(column.clone());
    let mut harness = harness(state);
    harness.get_by_label("CustomerID").click();
    harness.run_steps(2);
    harness.key_press(Key::Space);
    harness.run_steps(2);
    assert!(!harness.state().draft.columns[0].show);
    assert_eq!(harness.state().draft.columns[1], column);
    assert_eq!(harness.state().draft.columns[0].criteria, column.criteria);
    assert_eq!(harness.state().draft.columns[0].alias, column.alias);

    harness.key_press(Key::Plus);
    harness.run_steps(2);
    assert_eq!(harness.state().draft.columns[0], column);
    harness.key_press(Key::Minus);
    harness.run_steps(2);
    assert!(!harness.state().draft.columns[0].show);
    assert!(harness.state().draft.columns[1].show);

    for (key, pressed) in [
        (Key::Space, true),
        (Key::Space, false),
        (Key::Space, true),
        (Key::Space, false),
        (Key::ArrowDown, true),
        (Key::ArrowDown, false),
    ] {
        harness.event(Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
    }
    harness.run_steps(2);
    assert!(
        !harness.state().draft.columns[0].show,
        "two complete Space presses in one frame must toggle the original field twice"
    );
    assert_eq!(harness.state().draft.columns.len(), 2);
    assert!(harness.get_by_label("Field 00").is_focused());

    harness.key_press(Key::End);
    harness.run_steps(3);
    let last = harness.get_by_label("Field 23");
    assert!(last.is_focused());
    assert!(last.rect().bottom() < 180.0);
    harness.key_press(Key::Plus);
    harness.run_steps(2);
    assert_eq!(harness.state().draft.columns.len(), 3);
    assert_eq!(
        harness.state().draft.columns[2].expression,
        DesignExpression::Field(DesignField {
            source_id: 1,
            column: "Field 23".into(),
        })
    );
    harness.key_press(Key::ArrowUp);
    harness.run_steps(2);
    assert!(harness.get_by_label("Field 22").is_focused());
    harness.key_press(Key::Home);
    harness.run_steps(3);
    assert!(harness.get_all_by_label("*").any(|node| node.is_focused()));
    harness.key_press(Key::ArrowDown);
    harness.run_steps(2);
    assert!(
        harness.get_by_label("CustomerID").is_focused(),
        "Home followed by Down must stay in the current source list"
    );
    harness.key_press(Key::Tab);
    harness.run_steps(2);
    assert!(harness.get_all_by_label("*").any(|node| node.is_focused()));
    harness.key_press(Key::ArrowDown);
    harness.run_steps(2);
    assert!(harness.get_by_label("OrderCustomer").is_focused());
    harness.key_press(Key::Space);
    harness.run_steps(2);
    assert_eq!(
        harness.state().draft.columns[3].expression,
        DesignExpression::Field(DesignField {
            source_id: 2,
            column: "OrderCustomer".into(),
        })
    );
}

#[test]
fn activation_refuses_repeats_disabled_sources_and_column_limits_without_crossing_text_input() {
    let mut harness = harness(session());
    harness.get_by_label("CustomerID").click();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    assert!(harness.state().draft.columns.is_empty());
    harness.event(Event::Key {
        key: Key::Space,
        physical_key: Some(Key::Space),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    harness.run_steps(1);
    assert_eq!(harness.state().draft.columns.len(), 1);
    let original = harness.state().draft.clone();
    harness.event(Event::Key {
        key: Key::Space,
        physical_key: Some(Key::Space),
        pressed: true,
        repeat: true,
        modifiers: Modifiers::NONE,
    });
    harness.run_steps(2);
    assert_eq!(harness.state().draft, original);
    harness.event(Event::Key {
        key: Key::Space,
        physical_key: Some(Key::Space),
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    harness.run_steps(1);

    harness.state_mut().draft.columns = (0..32)
        .map(|_| DesignColumn::field(2, "OrderCustomer".into()))
        .collect();
    harness.key_press(Key::Plus);
    harness.run_steps(2);
    assert_eq!(harness.state().draft.columns.len(), 32);
    assert!(harness.state().error.is_some());

    let mut disabled = Harness::builder().build_ui_state(
        |ui, session| {
            ui.add_enabled_ui(false, |ui| {
                source_area(ui, session, 220.0);
            });
        },
        session(),
    );
    crate::configure(&disabled.ctx);
    disabled.run_steps(3);
    disabled.get_by_label("CustomerID").click_accesskit();
    disabled.key_press(Key::Space);
    disabled.run_steps(2);
    assert!(disabled.state().draft.columns.is_empty());
    assert!(
        disabled
            .get_by_label("CustomerID")
            .accesskit_node()
            .is_disabled()
    );

    let mut text = Harness::builder().build_ui_state(
        |ui, state: &mut (Session, String)| {
            source_area(ui, &mut state.0, 220.0);
            ui.text_edit_singleline(&mut state.1);
        },
        (session(), String::new()),
    );
    crate::configure(&text.ctx);
    text.run_steps(3);
    text.get_by_role(egui::accesskit::Role::TextInput).focus();
    text.run_steps(1);
    text.event(Event::Text(" + - ".into()));
    text.run_steps(2);
    assert_eq!(text.state().1, " + - ");
    assert!(text.state().0.draft.columns.is_empty());
}
