use super::model::{budget, database};
use std::{io::Cursor, ops::Bound::Included};
use tablecraft_ace::{
    IndexCheckStatus, IndexProjection, IndexProjectionSelection, TextIndexSelection, check_table,
    read_table_index, read_table_index_projection, read_table_text_index_range,
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

#[test]
fn exact_french_wire_supports_full_checks_aliases_and_both_node_forms() {
    let stored = [
        Value::Null,
        Value::Text(String::new()),
        Value::Text("cote".into()),
        Value::Text("coté".into()),
        Value::Text("côte".into()),
        Value::Text("côté".into()),
        Value::Text("COTE".into()),
        Value::Text("cote".into()),
        Value::Text("côté".into()),
    ];

    for code in [5, 6] {
        for descending in [false, true] {
            for node in [None, Some(false), Some(true)] {
                let model = database(code, descending, node);
                let mut source = Cursor::new(model.bytes.clone());
                let checked = check_table(
                    &mut source,
                    CatalogScope::new(1),
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert!(
                    checked
                        .indexes()
                        .iter()
                        .all(|item| item.status() == IndexCheckStatus::Checked)
                );
                for name in ["ScoreKey", "ScoreAlias"] {
                    let rows = read_table_index(
                        &mut source,
                        CatalogScope::new(1),
                        "Items",
                        name,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let actual = rows
                        .rows()
                        .iter()
                        .map(|row| row[0].clone())
                        .collect::<Vec<_>>();
                    let expected = model
                        .order
                        .iter()
                        .map(|row| Value::Int32(*row as i32 + 1))
                        .collect::<Vec<_>>();
                    assert_eq!(actual, expected);
                    for (row, original) in rows.rows().iter().zip(&model.order) {
                        assert_eq!(row[1], stored[*original]);
                    }
                }
                assert_eq!(source.into_inner(), model.bytes);
            }
        }
    }
}

#[test]
fn null_and_physical_french_ranges_preserve_ties_and_hidden_key_validation() {
    for descending in [false, true] {
        let model = database(6, descending, Some(true));
        let mut source = Cursor::new(model.bytes.clone());
        let bounds = if descending {
            ("coté", "côte")
        } else {
            ("côte", "coté")
        };
        for (selection, expected) in [
            (TextIndexSelection::Null, vec![Value::Int32(1)]),
            (
                TextIndexSelection::Range {
                    start: Included(bounds.0),
                    end: Included(bounds.1),
                },
                if descending {
                    vec![Value::Int32(4), Value::Int32(5)]
                } else {
                    vec![Value::Int32(5), Value::Int32(4)]
                },
            ),
            (
                TextIndexSelection::Range {
                    start: Included("cote"),
                    end: Included("cote"),
                },
                vec![Value::Int32(3), Value::Int32(7), Value::Int32(8)],
            ),
        ] {
            let rows = read_table_text_index_range(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "ScoreKey",
                selection,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                rows.rows()
                    .iter()
                    .map(|row| row[0].clone())
                    .collect::<Vec<_>>(),
                expected
            );
            let projected = read_table_index_projection(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "ScoreKey",
                IndexProjection {
                    columns: &["Id"],
                    selection: IndexProjectionSelection::Text(selection),
                },
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                projected
                    .rows()
                    .iter()
                    .map(|row| row[0].clone())
                    .collect::<Vec<_>>(),
                expected
            );
        }
        assert_eq!(source.into_inner(), model.bytes);
    }
}
