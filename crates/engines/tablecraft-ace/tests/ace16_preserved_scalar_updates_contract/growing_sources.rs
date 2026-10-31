//! Original-only/public-reader gates, before any growing producer is installed.
#[path = "growing_sources/api.rs"]
mod api;
#[path = "growing_sources/model.rs"]
pub(crate) mod model;

use super::{SCOPE, budget, model as rich};
use std::io::Cursor;
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, inspect_table_indexes, inspect_table_schema,
    read_attachment_column, read_stored_int32_calculation, read_table_index_projection,
    read_table_projection,
};
use tablecraft_core::values::Value;

fn qualify(bytes: &[u8], values: [Option<&str>; 3]) {
    let schema = inspect_table_schema(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.columns().len(), 16);
    assert_eq!(schema.declared_rows(), 3);
    assert_eq!(schema.columns()[1].name(), "Short_Text");
    let projected = read_table_projection(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &["ID", "Short_Text"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let expected = values
        .into_iter()
        .enumerate()
        .map(|(row, value)| {
            vec![
                Value::Int32((row + 1) as i32),
                value.map_or(Value::Null, |text| Value::Text(text.into())),
            ]
        })
        .collect::<Vec<_>>();
    assert_eq!(projected.rows(), expected);
    let indexes = inspect_table_indexes(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(indexes.indexes().len(), 3);
    for name in ["NumberKey", "PrimaryKey"] {
        let rows = read_table_index_projection(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            name,
            IndexProjection {
                columns: &["ID"],
                selection: IndexProjectionSelection::All,
            },
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            rows.rows(),
            [
                vec![Value::Int32(1)],
                vec![Value::Int32(2)],
                vec![Value::Int32(3)]
            ]
        );
    }
    let stored = read_stored_int32_calculation(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        "Calculated",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(stored.values(), &[Some(2), Some(4), Some(6)]);
    let attachments = read_attachment_column(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        "Attachment_Field",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        attachments
            .links()
            .parents()
            .iter()
            .map(|parent| (parent.key(), parent.child_count()))
            .collect::<Vec<_>>(),
        [(Some(10), 2), (Some(20), 1), (Some(30), 0)]
    );
}

#[test]
fn qualify_preserved_text_growth_complete_sources_before_growing_writer() {
    let mut images = 0;
    for shape in model::SHAPES {
        let case = model::source(shape);
        model::assert_placement(&case, shape);
        qualify(&case.before, [Some("alpha"), None, Some("omega")]);
        qualify(&case.expected, case.values);
        for page in 0..32 {
            if page != 12
                && !(matches!(shape, model::Shape::EndOfFile | model::Shape::ZeroNull)
                    && matches!(page, 1 | 11))
            {
                assert_eq!(
                    &case.before[page * 4096..(page + 1) * 4096],
                    &case.expected[page * 4096..(page + 1) * 4096]
                );
            }
        }
        assert_eq!(&case.before[31 * 4096..32 * 4096], &[0xc7; 4096]);
        if matches!(shape, model::Shape::Noops) {
            assert_eq!(case.before, case.expected);
            assert_eq!(case.changed, 0);
            assert!(case.destination.is_none());
        } else {
            assert_eq!(
                case.requests.len(),
                if matches!(shape, model::Shape::Mixed | model::Shape::MixedReverse) {
                    3
                } else {
                    1
                }
            );
            assert_eq!(
                case.changed,
                if matches!(shape, model::Shape::Mixed | model::Shape::MixedReverse) {
                    2
                } else {
                    1
                }
            );
            assert!(case.destination.is_some());
        }
        images += 2;
    }
    assert_eq!(images, 16);
    let first = model::source(model::Shape::Mixed);
    let reverse = model::source(model::Shape::MixedReverse);
    assert_eq!(first.before, reverse.before);
    assert_eq!(first.expected, reverse.expected);
    assert_eq!(
        first.requests.first().unwrap().row,
        reverse.requests.last().unwrap().row
    );
}

#[test]
fn preserve_growth_contradiction_sources_for_strongest_native_gate() {
    // These are deliberately not advertised as preservation-admitted sources.
    // A later native SOURCE sibling must prove exact alias/role/global refusals
    // before any API causality assertion can rely on them.
    for kind in [
        model::Contradiction::TerminalAlias,
        model::Contradiction::IndexOwnsParentData,
        model::Contradiction::GlobalSaysParentFree,
    ] {
        let bytes = model::contradiction(kind);
        let schema = inspect_table_schema(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(schema.declared_rows(), 3);
        assert_eq!(schema.columns().len(), 16);
        assert_eq!(bytes.len(), 32 * 4096);
    }
}

#[test]
fn header_five_rich_growth_derivatives_keep_unavailable_extended_declarations() {
    for shape in model::SHAPES {
        let case = model::source(shape);
        for mut bytes in [case.before, case.expected] {
            bytes[20] = 5;
            let schema = inspect_table_schema(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(schema.columns()[7].scalar_type().is_none());
            assert!(schema.columns()[8].scalar_type().is_none());
        }
    }
}
