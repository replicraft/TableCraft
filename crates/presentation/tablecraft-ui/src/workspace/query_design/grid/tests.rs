use super::*;
use egui::{Event, Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::query::{DesignFieldMetadata, DesignSource, build};

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
        Some("ID")
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
    assert_eq!(
        harness.state().0.draft.columns[0].expression,
        DesignExpression::Field(DesignField {
            source_id: 20,
            column: String::new(),
        })
    );

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
        Some("Display Name")
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
