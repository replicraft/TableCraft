//! Category visibility and pane controls preserve navigation authority.
use super::{Action, ObjectKind, ObjectTarget, Role, harness};
use crate::{Locale, Localizer, Theme};
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn categories_follow_filtered_membership_and_keep_collapsed_tables_reopenable() {
    let mut harness = harness();
    harness
        .state_mut()
        .0
        .database
        .as_mut()
        .unwrap()
        .queries
        .clear();
    harness.run();
    assert!(harness.query_by_label("Queries").is_none());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Tables")
            .is_some()
    );

    harness
        .state_mut()
        .0
        .database
        .as_mut()
        .unwrap()
        .tables
        .clear();
    harness.run();
    assert!(harness.query_by_label("Queries").is_none());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Tables")
            .is_none()
    );
    assert!(harness.state().1.is_empty());

    let mut harness = super::harness();
    harness.state_mut().0.database.as_mut().unwrap().queries[0].name = "Saved only".into();
    // Publish the changed object list before targeting coordinates from its
    // accessibility tree; pending layout must not supply a stale mouse target.
    harness.run();
    // "First" is also the disabled Record Navigator command. Retain the
    // actual enabled object-row identity so assertions cannot match that footer.
    let first_row_id = harness
        .get_all_by_role_and_label(Role::Button, "First")
        .find(|row| !row.accesskit_node().is_disabled())
        .unwrap()
        .accesskit_node()
        .id();
    let tables = harness.get_by_role_and_label(Role::Button, "Tables");
    assert_eq!(tables.accesskit_node().data().is_expanded(), Some(true));
    let bounds = tables.rect();
    tables.click();
    harness.run();
    assert_eq!(
        harness
            .get_by_role_and_label(Role::Button, "Tables")
            .accesskit_node()
            .data()
            .is_expanded(),
        Some(false),
        "a real pointer click at the current Tables header {bounds:?} must collapse it",
    );
    assert!(
        harness
            .get_all_by_role_and_label(Role::Button, "First")
            .all(|row| row.accesskit_node().id() != first_row_id)
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Tables")
            .is_some()
    );

    harness.state_mut().0.filter = "Saved only".into();
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Tables")
            .is_none()
    );
    assert!(harness.query_by_label("Queries").is_some());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Saved only")
            .is_some()
    );

    harness.state_mut().0.filter = "First".into();
    harness.run();
    assert!(harness.query_by_label("Queries").is_none());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Tables")
            .is_some()
    );
    assert!(
        harness
            .get_all_by_role_and_label(Role::Button, "First")
            .all(|row| row.accesskit_node().id() != first_row_id)
    );
    harness
        .get_by_role_and_label(Role::Button, "Tables")
        .click();
    harness.run();
    assert!(
        harness
            .get_all_by_role_and_label(Role::Button, "First")
            .any(|row| row.accesskit_node().id() == first_row_id)
    );

    harness.state_mut().0.filter = "No matching object".into();
    harness.run();
    assert!(harness.query_by_label("Queries").is_none());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Tables")
            .is_none()
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn system_only_categories_disappear_without_discarding_their_source_members() {
    let mut harness = harness();
    let original = {
        let database = harness.state().0.database.as_ref().unwrap();
        (
            database.tables.len(),
            database.queries.len(),
            database.document_revision,
        )
    };
    {
        let database = harness.state_mut().0.database.as_mut().unwrap();
        for table in &mut database.tables {
            table.system = true;
        }
        for query in &mut database.queries {
            query.system = true;
        }
    }
    harness.run();
    assert!(harness.query_by_label("Queries").is_none());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Tables")
            .is_none()
    );
    harness.state_mut().0.show_system = true;
    harness.run();
    assert!(harness.query_by_label("Queries").is_some());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Tables")
            .is_some()
    );
    assert_eq!(
        harness
            .get_all_by_role_and_label(Role::Button, "Second")
            .count(),
        2
    );
    let database = harness.state().0.database.as_ref().unwrap();
    assert_eq!(database.tables.len(), original.0);
    assert_eq!(database.queries.len(), original.1);
    assert_eq!(database.document_revision, original.2);
    assert!(harness.state().1.is_empty());
}

#[test]
fn localized_pane_control_is_compact_guarded_and_restores_the_same_selection() {
    for locale in Locale::ALL {
        for theme in [Theme::Light, Theme::Dark] {
            let mut harness = harness();
            let localizer = Localizer::new(locale).unwrap();
            let collapse_label = localizer.text("navigation-collapse-pane");
            let expand_label = localizer.text("navigation-expand-pane");
            let tables_label = localizer.text("navigation-tables");
            harness.state_mut().0.localizer = localizer;
            crate::theme::set_theme(&harness.ctx, theme);
            harness.run();
            let heading = harness.state().0.localizer.text("navigation-all-objects");
            let search = harness
                .state()
                .0
                .localizer
                .text("navigation-search-objects");
            let heading_left = harness
                .output()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == heading => {
                        Some(text.pos.x + text.galley.rect.left())
                    }
                    _ => None,
                })
                .expect("painted Navigation Pane heading");
            let search_left = harness
                .get_by_role_and_label(Role::TextInput, &search)
                .rect()
                .left();
            assert!(
                (heading_left - search_left).abs() <= 2.0,
                "{locale:?}/{theme:?}: heading must align with search, not center in its reserved width",
            );
            let target = ObjectTarget {
                document_revision: 17,
                kind: ObjectKind::Table {
                    name: "Second".into(),
                },
            };
            assert!(
                harness
                    .state_mut()
                    .0
                    .select_navigation_object(target.clone())
            );
            harness.state_mut().0.filter = "Second".into();
            harness.run();
            harness
                .get_by_role_and_label(Role::Button, &tables_label)
                .click();
            harness.run();
            assert_eq!(
                harness
                    .get_by_role_and_label(Role::Button, &tables_label)
                    .accesskit_node()
                    .data()
                    .is_expanded(),
                Some(false)
            );
            let control = harness.get_by_role_and_label(Role::Button, &collapse_label);
            assert!(control.rect().width() <= 20.0);
            assert!(control.rect().height() <= 20.0);
            assert!(!control.accesskit_node().is_disabled());
            control.click();
            harness.run();
            assert!(!harness.state().0.navigation_visible());
            assert!(
                harness
                    .query_by_role_and_label(Role::Button, &collapse_label)
                    .is_none()
            );
            assert_eq!(
                harness.state().0.selected_navigation_object(),
                Some(target.clone())
            );
            assert!(harness.state().1.is_empty());
            let expand = harness.get_by_role_and_label(Role::Button, &expand_label);
            assert!(expand.rect().width() <= 20.0);
            assert!(expand.rect().height() <= 20.0);
            assert!(!expand.accesskit_node().is_disabled());
            harness.state_mut().0.busy = true;
            harness.run_steps(3);
            assert!(
                harness
                    .get_by_role_and_label(Role::Button, &expand_label)
                    .accesskit_node()
                    .is_disabled()
            );
            harness
                .get_by_role_and_label(Role::Button, &expand_label)
                .click();
            harness.run_steps(3);
            assert!(!harness.state().0.navigation_visible());
            assert_eq!(
                harness.state().0.selected_navigation_object(),
                Some(target.clone())
            );
            assert!(harness.state().1.is_empty());
            harness.state_mut().0.busy = false;
            harness.run();
            harness
                .get_by_role_and_label(Role::Button, &expand_label)
                .click();
            harness.run();
            assert!(harness.state().0.navigation_visible());
            assert!(
                harness
                    .query_by_role_and_label(Role::Button, &expand_label)
                    .is_none()
            );
            assert_eq!(
                harness
                    .get_by_role_and_label(Role::Button, &tables_label)
                    .accesskit_node()
                    .data()
                    .is_expanded(),
                Some(false)
            );
            assert_eq!(
                harness
                    .get_all_by_role_and_label(Role::Button, "Second")
                    .count(),
                1,
                "only the saved query is rendered while Tables remains collapsed"
            );
            assert_eq!(
                harness.state().0.selected_navigation_object(),
                Some(target.clone())
            );
            assert_eq!(harness.state().0.filter, "Second");

            harness.state_mut().0.busy = true;
            // Busy deliberately animates the progress indicator indefinitely.
            harness.run_steps(3);
            assert!(
                harness
                    .get_by_role_and_label(Role::Button, &collapse_label)
                    .accesskit_node()
                    .is_disabled()
            );
            harness
                .get_by_role_and_label(Role::Button, &collapse_label)
                .click();
            harness.run_steps(3);
            assert!(harness.state().0.navigation_visible());
            assert_eq!(harness.state().0.selected_navigation_object(), Some(target));
            assert!(harness.state().1.is_empty());
            assert_eq!(
                harness.state().0.localizer.diagnostics().missing_messages,
                0
            );
            assert_eq!(
                harness.state().0.localizer.diagnostics().english_fallbacks,
                0
            );
        }
    }
}

#[test]
fn table_and_query_categories_collapse_independently_and_retain_filtered_state() {
    let mut harness = harness();
    let rows: Vec<_> = harness
        .get_all_by_role_and_label(Role::Button, "Second")
        .map(|row| row.accesskit_node().id())
        .collect();
    assert_eq!(rows.len(), 2);
    let table_row = rows[0];
    let query_row = rows[1];
    let target = ObjectTarget {
        document_revision: 17,
        kind: ObjectKind::SavedQuery {
            object_id: 91,
            name: "Second".into(),
        },
    };
    assert!(
        harness
            .state_mut()
            .0
            .select_navigation_object(target.clone())
    );
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Queries")
        .click();
    harness.run();
    let visible: Vec<_> = harness
        .get_all_by_role_and_label(Role::Button, "Second")
        .map(|row| row.accesskit_node().id())
        .collect();
    assert_eq!(visible, [table_row]);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::Button, "Tables")
            .accesskit_node()
            .data()
            .is_expanded(),
        Some(true)
    );

    harness
        .get_by_role_and_label(Role::Button, "Tables")
        .click();
    harness.run();
    assert_eq!(
        harness
            .query_all_by_role_and_label(Role::Button, "Second")
            .count(),
        0
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::Button, "Queries")
            .accesskit_node()
            .data()
            .is_expanded(),
        Some(false)
    );
    harness.state_mut().0.filter = "First".into();
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Queries")
            .is_none()
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::Button, "Tables")
            .accesskit_node()
            .data()
            .is_expanded(),
        Some(false)
    );

    harness.state_mut().0.filter = "Second".into();
    harness.run();
    assert_eq!(
        harness
            .get_by_role_and_label(Role::Button, "Queries")
            .accesskit_node()
            .data()
            .is_expanded(),
        Some(false)
    );
    harness
        .get_by_role_and_label(Role::Button, "Queries")
        .click();
    harness.run();
    let visible: Vec<_> = harness
        .get_all_by_role_and_label(Role::Button, "Second")
        .map(|row| row.accesskit_node().id())
        .collect();
    assert_eq!(visible, [query_row]);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::Button, "Tables")
            .accesskit_node()
            .data()
            .is_expanded(),
        Some(false)
    );
    assert_eq!(harness.state().0.selected_navigation_object(), Some(target));
    assert!(
        harness.state().1.is_empty(),
        "category changes never open or execute an object"
    );
}

#[test]
fn keyboard_category_transitions_invalidate_a_prior_pointer_pair_without_retargeting_rows() {
    for queries in [false, true] {
        let mut harness = harness();
        let (name, category, occurrence) = if queries {
            ("Second", "Queries", 1)
        } else {
            ("First", "Tables", 0)
        };
        let row = harness
            .get_all_by_role_and_label(Role::Button, name)
            .filter(|row| !row.accesskit_node().is_disabled())
            .nth(occurrence)
            .unwrap();
        let row_id = row.accesskit_node().id();
        let position = row.rect().center();
        row.click();
        harness.run_steps(1);
        let first_time = harness.ctx.input(|input| input.time);
        let selected = harness.state().0.selected_navigation_object();
        assert!(selected.is_some());
        assert!(harness.state().1.is_empty());

        // AccessKit activation emits no unrelated mouse release to clear the
        // pending pair. The category's actual expansion state must invalidate it.
        harness
            .get_by_role_and_label(Role::Button, category)
            .click_accesskit();
        harness.run_steps(1);
        assert!(
            harness
                .get_all_by_role_and_label(Role::Button, name)
                .all(|row| row.accesskit_node().id() != row_id)
        );
        harness
            .get_by_role_and_label(Role::Button, category)
            .click_accesskit();
        harness.run_steps(1);
        let restored = harness
            .get_all_by_role_and_label(Role::Button, name)
            .find(|row| row.accesskit_node().id() == row_id)
            .unwrap();
        assert_eq!(restored.rect().center(), position);
        restored.click();
        harness.run_steps(1);
        let elapsed = harness.ctx.input(|input| input.time) - first_time;
        let delay = harness
            .ctx
            .options(|options| options.input_options.max_double_click_delay);
        assert!(
            elapsed < delay,
            "the refusal must come from scope, not an expired click"
        );
        assert!(
            harness.state().1.is_empty(),
            "restoring a group cannot turn the old single click into Open"
        );
        assert_eq!(harness.state().0.selected_navigation_object(), selected);

        // A subsequent genuine second click on this unchanged live row still
        // opens exactly its original target through the ordinary activation path.
        harness
            .get_all_by_role_and_label(Role::Button, name)
            .find(|row| row.accesskit_node().id() == row_id)
            .unwrap()
            .click();
        harness.run_steps(1);
        assert!(
            matches!(harness.state().1.as_slice(), [Action::OpenObject { target }]
            if Some(target.clone()) == selected)
        );
    }
}
