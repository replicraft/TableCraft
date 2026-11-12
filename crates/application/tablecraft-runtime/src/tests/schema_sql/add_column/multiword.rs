//! Compound Text aliases feed the same unadorned declaration without gallery defaults.
use super::{inspect, review, same};
use crate::tests::{request, stored::opened, terminal};
use crate::{Command, Error, Response, SaveOutcome, SaveSchemaChange, SaveWorker};
use std::{fs, io::Cursor};
use tablecraft_ace::{inspect_table_schema, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    declared_fields, typed_fields::Kind, unadorned_fields,
};

#[test]
fn compound_text_add_saves_seven_native_utf16_units_and_historical_null_without_properties() {
    for alias in [
        "CHARACTER VARYING",
        "NATIONAL CHARACTER",
        "NATIONAL CHAR",
        "NATIONAL CHARACTER VARYING",
        "NATIONAL CHAR VARYING",
    ] {
        let mut source = declared_fields::source();
        for (position, mask) in [1, 27, 152, 253].into_iter().enumerate() {
            source.bytes[110 + position] = [9, 4, 0, 1][position] ^ mask;
        }
        let expected = unadorned_fields::after(
            &source.bytes,
            source.definition,
            "AddedValue",
            Kind::ShortText {
                units: 7,
                compression: false,
            },
            [9, 4, 0, 1],
        );
        let (_directory, path, worker, revision) = opened(&source.bytes);
        let target = inspect(
            &worker,
            revision,
            &format!("ALTER TABLE Items ADD COLUMN AddedValue {alias}(7)"),
        );
        let Response::FieldAdditionReviewed { capability, .. } = review(&worker, &target).unwrap()
        else {
            panic!("actual unadorned Text review");
        };
        let witness = capability.reopen_witness();
        same(witness.expected_bytes(), &expected);
        same(&fs::read(&path).unwrap(), &source.bytes);
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_field_addition(capability, crate::journal_path(&path))
            .unwrap();
        let outcome = terminal(&saves).outcome;
        assert!(matches!(
            outcome,
            SaveOutcome::Saved { receipt }
                if receipt.schema_change == Some(SaveSchemaChange::ColumnAdded)
                    && receipt.changed_rows.is_none()
                    && receipt.matched_rows.is_none()
        ));
        assert!(saves.acknowledge(id));
        assert!(matches!(
            request(&worker, Command::OpenColumnAdded { witness }),
            Ok(Response::Opened { .. })
        ));
        same(&fs::read(&path).unwrap(), &expected);
        let mut budget = ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX);
        let schema = inspect_table_schema(
            &mut Cursor::new(&expected),
            CatalogScope::new(31910),
            "Items",
            &mut budget,
            &|| false,
        )
        .unwrap();
        assert_eq!(schema.columns()[1].native_type(), 10);
        assert_eq!(schema.columns()[1].storage_length(), 14);
        let rows = read_table(
            &mut Cursor::new(&expected),
            CatalogScope::new(31910),
            "Items",
            &mut budget,
            &|| false,
        )
        .unwrap();
        assert!(
            rows.rows()
                .iter()
                .all(|row| row.last() == Some(&Value::Null))
        );
        assert!(!crate::journal_path(&path).exists());
    }
}

#[test]
fn binary_compound_syntax_does_not_grant_unsupported_add_storage_authority() {
    let source = declared_fields::source();
    let (_directory, path, worker, revision) = opened(&source.bytes);
    for alias in ["BINARY VARYING", "BIT VARYING"] {
        let target = inspect(
            &worker,
            revision,
            &format!("ALTER TABLE Items ADD COLUMN AddedValue {alias}(16)"),
        );
        assert!(matches!(
            review(&worker, &target),
            Err(Error::SchemaSqlUnsupported)
        ));
        same(&fs::read(&path).unwrap(), &source.bytes);
        assert!(!crate::journal_path(&path).exists());
    }
}
