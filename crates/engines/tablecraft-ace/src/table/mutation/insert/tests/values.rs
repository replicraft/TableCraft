use super::model;
use super::model::NewRow;
use crate::read_table;
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::insert_seed as source_model;
use tablecraft_testkit::access::insert_seed::PAGE;

#[test]
fn complete_rows_preserve_null_empty_binary_and_physical_primary_aliases() {
    for code in [5, 6] {
        for alias in [false, true] {
            let before = source_model::source(code, alias, false, None);
            let indexes = if alias { 2 } else { 1 };
            model::checked(&before, 3, indexes);
            let empty = model::capture(&before)
                .preview_ace16_insert_rows("Items", &[], &mut model::budget(), &|| false)
                .unwrap();
            assert_eq!(empty.changed_rows(), 0);
            assert_eq!(empty.candidate_bytes(), before);

            let rows = [
                NewRow {
                    id: 10,
                    binary: None,
                },
                NewRow {
                    id: 11,
                    binary: Some(Vec::new()),
                },
                NewRow {
                    id: 12,
                    binary: Some(vec![0, 128, 255]),
                },
            ];
            let requests = rows.iter().map(NewRow::request).collect::<Vec<_>>();
            let (expected, references) = model::expected(&before, &rows);
            assert_eq!(references, [(12 << 8) | 3, (12 << 8) | 4, (12 << 8) | 5]);
            let candidate = model::capture(&before)
                .preview_ace16_insert_rows("Items", &requests, &mut model::budget(), &|| false)
                .unwrap();
            assert_eq!(candidate.changed_rows(), 3);
            assert_eq!(candidate.candidate_bytes(), expected);
            model::checked(candidate.candidate_bytes(), 6, indexes);
            let actual = read_table(
                &mut Cursor::new(candidate.candidate_bytes()),
                CatalogScope::new(1),
                "Items",
                &mut model::budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                &actual.rows()[3..],
                &[
                    vec![Value::Int32(10), Value::Null],
                    vec![Value::Int32(11), Value::Binary(Vec::new())],
                    vec![Value::Int32(12), Value::Binary(vec![0, 128, 255])],
                ]
            );
        }
    }
}

#[test]
fn maximum_binary_batch_composes_two_visible_eof_pages_and_shared_map_bytes() {
    for code in [5, 6] {
        let before = source_model::source(code, true, true, None);
        model::checked(&before, 3, 2);
        let rows = (0..20)
            .map(|position| NewRow {
                id: 100 + position,
                binary: Some(vec![position as u8; 255]),
            })
            .collect::<Vec<_>>();
        let requests = rows.iter().map(NewRow::request).collect::<Vec<_>>();
        let (expected, references) = model::expected(&before, &rows);
        assert_eq!(expected.len(), before.len() + 2 * PAGE);
        assert_eq!(references[0], 16 << 8);
        assert_eq!(references[14], (16 << 8) | 14);
        assert_eq!(references[15], 17 << 8);
        let candidate = model::capture(&before)
            .preview_ace16_insert_rows("Items", &requests, &mut model::budget(), &|| false)
            .unwrap();
        assert_eq!(candidate.changed_rows(), rows.len());
        assert_eq!(candidate.candidate_bytes(), expected);
        model::checked(candidate.candidate_bytes(), 23, 2);
    }
}

#[test]
fn slot254_is_visible_then_count255_forces_a_fresh_visible_root() {
    for code in [5, 6] {
        let before = source_model::source(code, false, false, None);
        model::checked(&before, 3, 1);
        let rows = (0..253)
            .map(|position| NewRow {
                id: 100 + position,
                binary: None,
            })
            .collect::<Vec<_>>();
        let requests = rows.iter().map(NewRow::request).collect::<Vec<_>>();
        let (expected, references) = model::expected(&before, &rows);
        assert_eq!(references[251], (12 << 8) | 254);
        assert_eq!(references[252], 16 << 8);
        assert_eq!(expected.len(), before.len() + PAGE);
        let candidate = model::capture(&before)
            .preview_ace16_insert_rows("Items", &requests, &mut model::budget(), &|| false)
            .unwrap();
        assert_eq!(candidate.candidate_bytes(), expected);
        model::checked(candidate.candidate_bytes(), 256, 1);
    }
}
