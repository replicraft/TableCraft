//! Whole batch images qualify before any typed omission consumer exists.
#[path = "typed_omissions/api.rs"]
mod api;
#[path = "typed_omissions/model.rs"]
mod model;

use super::{PAGE, budget, checked};
use model::{Kind, Row};
use std::io::Cursor;
use tablecraft_ace::{
    IndexCheckStatus, NativeImage, inspect_table_index_counter, inspect_table_schema, read_table,
};
use tablecraft_model::CatalogScope;

fn qualify(bytes: &[u8], rows: &[Row], generated: bool, kind: Kind) {
    checked(bytes, rows.len());
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let wanted = rows.iter().map(|row| row.values(kind)).collect::<Vec<_>>();
    assert!(table.rows() == wanted, "complete literal batch typed rows");
    let schema = inspect_table_schema(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let fields = schema
        .columns()
        .iter()
        .map(|column| {
            (
                column.name(),
                column.number(),
                column.native_type(),
                column.storage_length(),
                column.flags(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        fields,
        [
            ("Id", 0, 4, 4, 3),
            ("Score", 1, 4, 4, if generated { 7 } else { 3 }),
            (
                "Other",
                2,
                if matches!(kind, Kind::Clock) { 8 } else { 4 },
                if matches!(kind, Kind::Clock) { 8 } else { 4 },
                3
            ),
        ]
    );
    let report = tablecraft_ace::check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.indexes().len(), 1);
    for index in report.indexes() {
        assert_eq!(index.status(), IndexCheckStatus::Checked);
        assert_eq!(index.index().data_number(), 0);
        let history = inspect_table_index_counter(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            index.index().name(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(history.value(), rows.len() as u32);
    }
    if generated {
        let image = NativeImage::capture(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let binding = image
            .prepare_ace16_incrementing_insert_binding("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(binding.generated_position(), 1);
        assert_eq!(binding.generated_name(), "Score");
        assert_eq!(binding.policy().expected_last(), rows.last().unwrap().score);
    }
}

#[test]
fn qualify_typed_batch_omission_sources_and_complete_expected_before_consumer() {
    let mut pairs = 0;
    for code in [5, 6] {
        for generated in [false, true] {
            for kind in [Kind::Integer, Kind::Clock] {
                for count in [2, 3] {
                    let case = model::case(code, generated, kind, count);
                    qualify(&case.before, &case.original, generated, kind);
                    let mut rows = case.original.clone();
                    rows.extend(case.added.clone());
                    qualify(&case.expected, &rows, generated, kind);
                    assert_eq!(case.before.len(), case.expected.len());
                    assert_eq!(case.before.len() % PAGE, 0);

                    let mut input = csv::ReaderBuilder::new()
                        .delimiter(b'\t')
                        .has_headers(false)
                        .from_reader(case.requests.as_slice());
                    let records = input.records().map(Result::unwrap).collect::<Vec<_>>();
                    assert_eq!(records.len(), count);
                    for record in records {
                        assert_eq!(record.len(), if generated { 4 } else { 6 });
                        let last = record.len() - 2;
                        if matches!(&record[last], "omitted" | "null") {
                            assert_eq!(&record[last + 1], "");
                        }
                    }
                    pairs += 1;
                }
            }
        }
    }
    assert_eq!(pairs, 16);
}
