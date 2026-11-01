//! Explicit source reads of independently generated originals and literal edits.
use crate::{
    IndexCheckStatus, NativeImage, PayloadPresence, check_table, check_table_constraints,
    read_table,
};
use sha2::{Digest, Sha256};
use std::{env, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(124);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn value(raw: &str) -> Value {
    if raw == "NULL" {
        Value::Null
    } else {
        Value::Int32(raw.parse().unwrap())
    }
}

#[test]
#[ignore = "requires explicitly supplied, pinned independently generated source artifacts"]
fn genuine_targets_and_literal_after_views_pass_native_read_checks() {
    let directory = PathBuf::from(
        env::var_os("TABLECRAFT_GENUINE_TARGET_EXPECTATIONS_DIR")
            .expect("set the explicit genuine expected-artifact directory"),
    );
    let mut inventory = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_path(directory.join("native-views.tsv"))
        .unwrap();
    assert_eq!(
        inventory.headers().unwrap(),
        &csv::StringRecord::from(vec![
            "id", "header", "file", "id0", "score0", "guard0", "sha256",
        ])
    );

    let mut count = 0;
    for row in inventory.records() {
        let row = row.unwrap();
        let header: u8 = row[1].parse().unwrap();
        assert!(matches!(header, 5 | 6));
        assert!(row[0].starts_with(&format!("h{header}-")));
        assert!(
            row[0]
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        );
        assert_eq!(row[2], format!("{}.accdb", &row[0]));
        let bytes = fs::read(directory.join(&row[2])).unwrap();
        let digest: String = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(digest, row[6]);
        let expected = vec![
            vec![value(&row[3]), value(&row[4]), value(&row[5])],
            vec![Value::Int32(2), Value::Int32(0), Value::Int32(1)],
            vec![Value::Int32(3), Value::Int32(1), Value::Int32(2)],
        ];
        let image = NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
            .unwrap();
        let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap_or_else(|error| panic!("{}: {error}", &row[0]));
        assert_eq!(table.columns().len(), 3);
        for (column, expected) in table.columns().iter().zip(["Id", "Score", "Guard"]) {
            assert_eq!(column.name(), expected);
        }
        assert_eq!(table.rows(), expected, "{}", &row[0]);
        assert_eq!(
            table.schema().definition_page(),
            if header == 5 { 89 } else { 106 }
        );

        let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(checked.rows(), 3);
        assert_eq!(checked.indexes().len(), 4);
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        let constraints =
            check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(constraints.rows(), 3);

        // These are inert observations. They do not establish exhaustive native
        // hook discovery or grant a normal save capability.
        let coverage = image
            .inspect_table_macro_coverage(table.schema().object_id(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(coverage.body().presence(), PayloadPresence::Absent);
        assert_eq!(coverage.module().presence(), PayloadPresence::Absent);
        assert_eq!(coverage.table_extra().presence(), PayloadPresence::Absent);
        assert!(coverage.children().is_empty());
        assert_eq!(image.as_bytes(), bytes);
        count += 1;
    }
    assert_eq!(count, 12);
}
