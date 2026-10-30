//! SOURCE-only named input intent and complete images, before any consumer.
#[path = "named/api.rs"]
mod api;
#[path = "named/model.rs"]
mod model;

use super::{budget, model as original_model, qualify};
use std::io::Cursor;
use tablecraft_ace::{inspect_table_index_counter, read_table};
use tablecraft_model::CatalogScope;

#[test]
fn qualify_named_batch_sources_and_expected_before_mapping() {
    let mut pairs = 0;
    for code in [5, 6] {
        for generated in [false, true] {
            for kind in [original_model::Kind::Integer, original_model::Kind::Clock] {
                for count in [2, 3] {
                    qualify_case(model::reordered(code, generated, kind, count));
                    pairs += 1;
                }
                qualify_case(model::missing_other(code, generated, kind));
                pairs += 1;
            }
            qualify_case(model::missing_nullable_id(code, generated));
            pairs += 1;
        }
    }
    assert_eq!(pairs, 28);
}

fn qualify_case(case: model::Case) {
    qualify(&case.before, &case.original, case.generated, case.kind);
    let table = read_table(
        &mut Cursor::new(&case.expected),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        table.rows() == case.final_values,
        "complete named-input typed rows"
    );
    super::super::checked(&case.expected, case.final_values.len());
    let counter = inspect_table_index_counter(
        &mut Cursor::new(&case.expected),
        CatalogScope::new(1),
        "Items",
        "ScoreKey",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(counter.value() as usize, case.final_values.len());

    assert!(!case.fields.is_empty());
    assert!(!case.generated || !case.fields.contains(&"Score"));
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(case.requests.as_slice());
    let records = reader.records().map(Result::unwrap).collect::<Vec<_>>();
    assert_eq!(records.len(), case.final_values.len() - case.original.len());
    assert!(records.iter().all(|row| row.len() == case.fields.len() * 2));
    assert_eq!(case.before.len(), case.expected.len());
    for page in 0..case.before.len() / 4096 {
        if !matches!(page, 10 | 12 | 13) {
            assert!(
                case.before[page * 4096..(page + 1) * 4096]
                    == case.expected[page * 4096..(page + 1) * 4096]
            );
        }
    }
}
