use super::*;
use egui::{Event, Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::query::{
    DesignField, DesignFieldMetadata, DesignSource, DesignTableMetadata, build,
};

fn session() -> Session {
    let mut session = Session::default();
    for (id, alias) in [(10, "Parent"), (20, "Child")] {
        session.draft.sources.push(DesignSource {
            id,
            table: "Contacts".into(),
            alias: Some(alias.into()),
        });
    }
    session.metadata.push(DesignTableMetadata {
        table: "Contacts".into(),
        fields: ["ID", "Display Name"]
            .into_iter()
            .map(|name| DesignFieldMetadata {
                name: name.into(),
                scalar_type: None,
            })
            .collect(),
    });
    session
        .draft
        .columns
        .push(DesignColumn::field(20, "ID".into()));
    session
}

fn harness(session: Session) -> Harness<'static, (Session, bool)> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(800.0, 400.0))
        .build_ui_state(
            |ui, state: &mut (Session, bool)| state.1 |= design_grid(ui, &mut state.0),
            (session, false),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
}

fn replace_field(harness: &mut Harness<'_, (Session, bool)>, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, "Field expression")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(Event::Paste(text.into()));
    harness.run_steps(3);
}

fn assert_input_focus_outline(harness: &Harness<'_, (Session, bool)>, label: &str) {
    let input = harness.get_by_role_and_label(Role::TextInput, label);
    let rect = input.rect();
    let accent = crate::theme::palette(&harness.ctx).accent;
    assert!(input.is_focused());
    assert!(
        harness.output().shapes.iter().any(|shape| {
            matches!(
                &shape.shape,
                egui::Shape::Rect(outline)
                    if outline.rect == rect
                        && outline.stroke.color == accent
                        && outline.stroke.width == 1.0
                        && outline.stroke_kind == egui::StrokeKind::Inside
            )
        }),
        "the actual focused input needs an outline independent of caret blink"
    );
}

#[test]
fn tab_and_shift_tab_visit_grid_controls_without_changing_text_or_source() {
    let mut harness = harness(session());
    let original = harness.state().0.draft.clone();
    harness
        .get_by_role_and_label(Role::TextInput, "Field expression")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::Tab);
    harness.run_steps(2);
    assert!(harness.get_by_role(Role::ComboBox).is_focused());
    harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    harness.run_steps(2);
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field expression")
            .is_focused()
    );
    assert_input_focus_outline(&harness, "Field expression");
    assert_eq!(harness.state().0.draft, original);
    assert!(!harness.state().1);
}

#[test]
fn focused_field_shortcut_menu_edits_alias_and_escape_returns_to_exact_field() {
    let mut harness = harness(session());
    let expression = harness.state().0.draft.columns[0].expression.clone();
    harness
        .get_by_role_and_label(Role::TextInput, "Field expression")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run_steps(2);
    assert!(egui::Popup::is_any_open(&harness.ctx));
    assert!(!harness.state().1);
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, "Output alias")
            .is_focused()
    );
    assert_input_focus_outline(&harness, "Output alias");
    harness.event(Event::Text("Keyboard alias".into()));
    harness.run_steps(2);
    assert_eq!(
        harness.state().0.draft.columns[0].alias.as_deref(),
        Some("Keyboard alias")
    );
    assert_eq!(harness.state().0.draft.columns[0].expression, expression);
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!egui::Popup::is_any_open(&harness.ctx));
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field expression")
            .is_focused()
    );

    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run_steps(2);
    harness.key_press(Key::Tab);
    harness.run_steps(2);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Remove column")
            .is_focused()
    );
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    assert!(harness.state().0.draft.columns.is_empty());
    assert!(!egui::Popup::is_any_open(&harness.ctx));
}

#[test]
fn local_field_text_keeps_the_owning_source_across_focus_alias_and_noop_replacement() {
    let mut harness = harness(session());
    let original = harness.state().0.draft.clone();
    let field = harness.get_by_role_and_label(Role::TextInput, "Field expression");
    assert_eq!(field.accesskit_node().value().as_deref(), Some("ID"));
    field.focus();
    harness.run_steps(2);
    assert_eq!(harness.state().0.draft, original);
    assert!(!harness.state().1);

    replace_field(&mut harness, "ID");
    assert_eq!(harness.state().0.draft, original);
    assert!(!harness.state().1);
    let sql = build(&harness.state().0.draft, &harness.state().0.metadata)
        .unwrap()
        .sql;
    assert!(sql.starts_with("SELECT [Child].[ID]"), "{sql}");

    // Changing an output alias is independent of the owning field source.
    harness.state_mut().0.draft.columns[0].alias = Some("Selected contact".into());
    harness.run_steps(2);
    assert_eq!(
        harness.state().0.draft.columns[0].expression,
        DesignExpression::Field(DesignField {
            source_id: 20,
            column: "ID".into(),
        })
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field expression")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("[Selected contact]: ID")
    );
}

#[test]
fn incremental_local_name_editing_keeps_source_without_transient_widget_state() {
    let mut harness = harness(session());
    harness
        .get_by_role_and_label(Role::TextInput, "Field expression")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.key_press(Key::Backspace);
    harness.run_steps(1);
    assert_eq!(harness.state().0.field_inputs.text(0), Some(""));
    assert!(harness.state().0.field_inputs.unresolved());

    for character in "display name".chars() {
        harness.event(Event::Text(character.to_string()));
        harness.run_steps(1);
        let DesignExpression::Field(field) = &harness.state().0.draft.columns[0].expression else {
            panic!("known local-name prefixes must retain their explicit table");
        };
        assert_eq!(field.source_id, 20);
        assert!(
            harness
                .get_all_by_label("Child")
                .any(|node| node.accesskit_node().role() == Role::Label),
            "the Table row must retain its owning alias during incremental input"
        );
    }
    assert_eq!(
        harness.state().0.draft.columns[0].expression,
        DesignExpression::Field(DesignField {
            source_id: 20,
            column: "Display Name".into(),
        })
    );
    harness.key_press(Key::Tab);
    harness.run_steps(1);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field expression")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("display name")
    );
    let sql = build(&harness.state().0.draft, &harness.state().0.metadata)
        .unwrap()
        .sql;
    assert!(sql.starts_with("SELECT [Child].[Display Name]"), "{sql}");

    replace_field(&mut harness, "ID");
    harness.key_press(Key::End);
    harness.event(Event::Text("x".into()));
    harness.run_steps(1);
    assert_eq!(
        harness.state().0.draft.columns[0].expression,
        DesignExpression::Expression("IDx".into())
    );
    harness.key_press(Key::Backspace);
    harness.run_steps(1);
    assert_eq!(
        harness.state().0.draft.columns[0].expression,
        DesignExpression::Expression("ID".into())
    );
    assert!(harness.query_by_label("Child").is_none());
    assert!(
        build(&harness.state().0.draft, &harness.state().0.metadata).is_err(),
        "after entering arbitrary-expression mode a correction must not guess either source"
    );
}

#[test]
fn a_completed_field_keeps_its_logical_self_join_source_after_blur() {
    let mut harness = harness(session());
    replace_field(&mut harness, "Selected: ID");
    harness.key_press(Key::Tab);
    harness.run_steps(2);
    assert_eq!(harness.state().0.field_inputs.source_hint(0), None);
    replace_field(&mut harness, "Selected: display name");
    assert_eq!(
        harness.state().0.draft.columns[0].expression,
        DesignExpression::Field(DesignField {
            source_id: 20,
            column: "Display Name".into()
        })
    );
    assert_eq!(
        harness.state().0.draft.columns[0].alias.as_deref(),
        Some("Selected")
    );
    assert!(
        build(&harness.state().0.draft, &harness.state().0.metadata)
            .unwrap()
            .sql
            .starts_with("SELECT [Child].[Display Name] AS [Selected]")
    );
}

#[test]
fn explicit_local_field_changes_keep_the_table_and_formulas_remain_checked_sql() {
    let mut harness = harness(session());
    replace_field(&mut harness, "display name");
    assert!(harness.state().1);
    assert_eq!(
        harness.state().0.draft.columns[0].expression,
        DesignExpression::Field(DesignField {
            source_id: 20,
            column: "Display Name".into(),
        })
    );
    let sql = build(&harness.state().0.draft, &harness.state().0.metadata)
        .unwrap()
        .sql;
    assert!(sql.starts_with("SELECT [Child].[Display Name]"), "{sql}");

    replace_field(&mut harness, "*");
    assert_eq!(
        harness.state().0.draft.columns[0].expression,
        DesignExpression::AllFields(Some(20))
    );
    harness.state_mut().1 = false;
    replace_field(&mut harness, "*");
    assert_eq!(
        harness.state().0.draft.columns[0].expression,
        DesignExpression::AllFields(Some(20))
    );
    assert!(!harness.state().1);
    let sql = build(&harness.state().0.draft, &harness.state().0.metadata)
        .unwrap()
        .sql;
    assert!(sql.starts_with("SELECT [Child].*"), "{sql}");

    replace_field(&mut harness, "[Parent].[ID] + 1");
    assert_eq!(
        harness.state().0.draft.columns[0].expression,
        DesignExpression::Expression("[Parent].[ID] + 1".into())
    );
    let sql = build(&harness.state().0.draft, &harness.state().0.metadata)
        .unwrap()
        .sql;
    assert!(sql.starts_with("SELECT ([Parent].[ID] + 1)"), "{sql}");

    replace_field(&mut harness, "ID");
    assert!(
        build(&harness.state().0.draft, &harness.state().0.metadata).is_err(),
        "an explicit unqualified formula remains ambiguous across the two aliases"
    );
}
