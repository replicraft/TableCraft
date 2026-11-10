//! Opt-in original-file journey without native windows or retained private values.
use crate::workspace::test_support::{finish_reads, opened};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_runtime::{Command, QueryPoolLimits, QueryRunOptions};
use tablecraft_testkit::support::access::registered_unlocode;
use tablecraft_ui::{ObjectKind, ObjectTarget};

#[test]
#[ignore = "requires explicit hash-pinned TABLECRAFT_UNLOCODE_SOURCE"]
fn original_ace2007_catalog_last_page_and_query_keep_the_source_unchanged() {
    let (source, original) = registered_unlocode::load_from_environment();
    let (_directory, path, original, mut harness) = opened(original);
    let database = harness.state().state.database.as_ref().unwrap();
    assert!(database.format.contains("Ace2007"));
    let revision = database.document_revision;
    let names = database
        .tables
        .iter()
        .filter(|table| table.local && !table.system)
        .map(|table| {
            // NO_CLONE: The host mutates its catalog while this scenario visits every original table.
            table.name.clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 5);
    assert!(
        harness.state().state.data.is_some(),
        "first table opens automatically"
    );

    let mut declarations = Vec::new();
    let mut largest = None;
    for name in names {
        harness.state_mut().open_object(ObjectTarget {
            document_revision: revision,
            kind: ObjectKind::Table { name },
        });
        finish_reads(&mut harness);
        harness
            .get_by_role_and_label(Role::Button, "View")
            .click_accesskit();
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::Button, "Design view")
            .click_accesskit();
        harness.run_steps(1);
        finish_reads(&mut harness);
        let schema = harness.state().state.schema.as_ref().unwrap();
        declarations.push((schema.declared_rows, schema.fields.len()));
        if schema.declared_rows == 100_969 {
            // NO_CLONE: Retain the selected table identity across later host publications.
            largest = Some((schema.table.clone(), schema.fields[0].name.clone()));
        }
        harness
            .get_by_role_and_label(Role::Button, "Datasheet view")
            .click_accesskit();
        harness.run_steps(1);
        finish_reads(&mut harness);
    }
    declarations.sort_unstable();
    assert_eq!(
        declarations,
        [(10, 2), (14, 2), (249, 2), (4_759, 3), (100_969, 12)]
    );

    let (largest, column) = largest.expect("independently recorded largest table");
    let sql = format!(
        "SELECT Count(*) AS Total FROM [{}]",
        largest.replace(']', "]]")
    );
    // NO_CLONE: The opened tab and the subsequent record-navigation request each own their table identity.
    harness.state_mut().open_object(ObjectTarget {
        document_revision: revision,
        kind: ObjectKind::Table {
            name: largest.clone(),
        },
    });
    finish_reads(&mut harness);
    harness
        .state_mut()
        .navigate_record(tablecraft_ui::RecordTarget {
            document_revision: revision,
            table: largest,
            parent_ordinal: 100_900,
            column,
        });
    finish_reads(&mut harness);
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.offset, 100_900);
    assert_eq!(page.rows.len(), 69);
    assert_eq!(page.columns.len(), 12);
    assert!(!page.has_more);

    let options = QueryRunOptions::with_limits(
        200_000,
        1_000,
        QueryPoolLimits::new(256 * 1024 * 1024, 200_000, 100_000_000).unwrap(),
        QueryPoolLimits::new(256 * 1024 * 1024, 1_000_000, 100_000_000).unwrap(),
        QueryPoolLimits::new(256 * 1024 * 1024, 0, 100_000_000).unwrap(),
        QueryPoolLimits::new(256 * 1024 * 1024, 0, 100_000_000).unwrap(),
    )
    .unwrap();
    harness
        .get_by_role_and_label(Role::Button, "New SQL query")
        .click_accesskit();
    harness.run_steps(2);
    harness.state_mut().submit(Command::Query { options, sql });
    finish_reads(&mut harness);
    let result = harness.state().state.query_data.as_ref().unwrap();
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0][0].text, "100969");
    assert!(result.row_ordinals.is_none());
    assert!(result.source_revision.is_none());

    assert_eq!(std::fs::read(path).unwrap(), original);
    assert_eq!(std::fs::read(source).unwrap(), original);
}
