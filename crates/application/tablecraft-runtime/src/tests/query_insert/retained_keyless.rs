//! SQL append selection retains the actual surviving field after a native DROP.
use super::{inspect, open, opened, request, save};
use crate::{Command, Response};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{field_changes, fields};

#[test]
fn values_and_select_append_to_native_field_one_without_recreating_retired_field_zero() {
    let source = fields::source(fields::Case::FirstNumber);
    let definition = (source.bytes.len() / 4096) as u32;
    let original = field_changes::keyless_first_field(&source.bytes, "Keyless");
    let before = field_changes::drop_field(&original, definition, source.catalog_root, "Id");

    for (sql, expected_value) in [
        ("INSERT INTO Keyless (Score) VALUES (77)", 77),
        (
            "INSERT INTO Keyless (Score) SELECT Score + 1 FROM Keyless",
            24,
        ),
    ] {
        let expected =
            field_changes::inserting::keyless_score(&before, definition, Some(expected_value));
        let (_directory, path, worker, revision) = opened(&before);
        let (target, parameters) = inspect(&worker, revision, sql);
        assert!(parameters.is_empty());

        let Response::QueryInsertReviewed { review, capability } = request(
            &worker,
            Command::ReviewQueryInsert {
                target,
                parameters: Vec::new(),
            },
        )
        .unwrap() else {
            panic!("expected a source-bound SQL append review");
        };
        assert_eq!(review.table, "Keyless");
        assert_eq!(review.document_revision, revision);
        assert_eq!(review.inserted_rows, 1);
        assert!(
            std::fs::read(&path).unwrap() == before,
            "review changed source bytes"
        );

        let bundle = crate::journal_path(&path);
        save(capability, bundle.clone(), 1);
        assert!(!bundle.exists());
        let actual = std::fs::read(&path).unwrap();
        assert_eq!(actual.len(), expected.len());
        assert_eq!(
            actual.iter().zip(&expected).position(|(a, b)| a != b),
            None,
            "first differing byte from the independent append image"
        );

        request(&worker, open(&path, "")).unwrap();
        let Response::Data {
            columns,
            rows,
            has_more,
            ..
        } = request(
            &worker,
            Command::Page {
                table: "Keyless".into(),
                offset: 0,
            },
        )
        .unwrap()
        else {
            panic!("expected fresh SQL append results");
        };
        assert!(!has_more);
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].name, "Score");
        assert_eq!(columns[0].native_number, Some(1));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0].text, "23");
        assert_eq!(rows[1][0].text, expected_value.to_string());
    }
}
