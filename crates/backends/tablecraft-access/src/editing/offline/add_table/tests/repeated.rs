//! Repeated real-file addition and first insertion retain every earlier table.
use super::{budget, definition, open};
use crate::editing::offline::{SchemaChange, test_support::assert_completion};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, add_table_source_available, read_table};
use tablecraft_core::values::Value;
use tablecraft_db::{InsertValue, OmittingInsertableTable};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_repeated_add_table;

#[test]
fn repeated_schema_saves_and_first_inserts_preserve_all_populated_predecessors() {
    let source = canonical_repeated_add_table::source();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("repeated.accdb");
    let bundle = directory.path().join("journal");
    let mut current = source.before;
    let mut rows = vec![
        ("Items", vec![Value::Int32(1), Value::Int32(17)]),
        ("More", vec![Value::Int32(1), Value::Text("Ada".into())]),
    ];
    fs::write(&path, &current).unwrap();

    for (table, text) in [("Third", "third row"), ("Fourth", "fourth row")] {
        let staged = open(&path)
            .plan_add_table_if_original(definition(table), &current, &mut budget(), &|| false)
            .unwrap();
        let (mut session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(
            receipt.schema_change(),
            Some(SchemaChange::TableAdded { column_count: 2 })
        );
        assert_completion(&receipt);

        // Read through the retained descriptor while Windows's exclusive
        // sharing still excludes an ordinary path observation.
        let slot = session.slot_mut().unwrap();
        let image = NativeImage::capture(
            slot.target.as_mut().unwrap(),
            slot.scope,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        if table == "Third" {
            assert_eq!(image.as_bytes(), source.after);
        }
        assert_eq!(
            &image.as_bytes()[7 * 4096..current.len()],
            &current[7 * 4096..]
        );
        for (old_table, expected) in &rows {
            let observed = read_table(
                &mut image.reader(),
                slot.scope,
                old_table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(observed.rows(), std::slice::from_ref(expected));
        }
        let added = read_table(
            &mut image.reader(),
            slot.scope,
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(added.rows().is_empty());

        let insertion = session
            .insert_table_if_original(table, image.as_bytes(), &mut budget(), &|| false)
            .unwrap()
            .plan_insert_with_omissions(
                vec![
                    InsertValue::Omitted,
                    InsertValue::Supplied(Value::Text(text.into())),
                ],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (session, receipt) = insertion.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.changed_rows(), Some(1));
        assert_eq!(receipt.schema_change(), None);
        assert_completion(&receipt);
        session.release().unwrap();

        let saved = fs::read(&path).unwrap();
        assert_eq!(&saved[7 * 4096..current.len()], &current[7 * 4096..]);
        current = saved;
        rows.push((table, vec![Value::Int32(1), Value::Text(text.into())]));
        assert!(!bundle.exists());
    }

    for (table, expected) in &rows {
        let observed = read_table(
            &mut Cursor::new(&current),
            CatalogScope::new(84),
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(observed.rows(), std::slice::from_ref(expected));
    }
    assert!(
        add_table_source_available(&current, CatalogScope::new(85), &mut budget(), &|| false)
            .unwrap()
    );
}
