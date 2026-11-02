//! Frozen explicit Score images; complete read/Source closure, no mutation plan.
use crate::{
    EditError, IndexCheckStatus, NativeImage, PropertyGroupKind, RuleCheckStatus, WriteBlocker,
    check_table, check_table_constraints, inspect_table_index_counter, inspect_table_properties,
    read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const SCOPE: CatalogScope = CatalogScope::new(242);
const MANIFEST_SHA: &str = "55cb6e02acf6caa93dad011fdcaad8f07465a3f4e4c3bed4beeb624edebaf912";
const VIEWS: &[(u32, &str, &str, &str, &str)] = &[
    (
        5,
        "Numbered",
        "before",
        "h5-numbered-before.accdb",
        "c6dc5d36f35335ea2d9cafbc5cf6500dc135ea41a4d3f60d9241ac9e55714d12",
    ),
    (
        5,
        "Numbered",
        "batch",
        "h5-numbered-batch.accdb",
        "92126d4eb2ad907587842e426c5a96893001e2c59f0cf24d7ec570a28941d6f4",
    ),
    (
        5,
        "Numbered",
        "score-null",
        "h5-numbered-score-null.accdb",
        "c11d3e88db0a5b62d46eb6176246bffd50939083861704ccbd361340df7050f9",
    ),
    (
        6,
        "Numbered",
        "before",
        "h6-numbered-before.accdb",
        "c6aae0208a2dfbd0b77f8867f49700718880ee761c7881df6ec5789843a8b456",
    ),
    (
        6,
        "Numbered",
        "batch",
        "h6-numbered-batch.accdb",
        "b1d3aa21d750cf4cfe45c990f70868b31ca7b9ebb253ebf4616fde6f4db5b291",
    ),
    (
        6,
        "Numbered",
        "score-null",
        "h6-numbered-score-null.accdb",
        "ebf3a25e37386dde110695c95abb53a9281b80df5408c7a861d544fbbafd80ef",
    ),
    (
        5,
        "Items",
        "before",
        "h5-random-before.accdb",
        "d45a6157d94c892b8963449796b30d5e54512de1894fa9def47c042b240c2638",
    ),
    (
        5,
        "Items",
        "change",
        "h5-random-change.accdb",
        "eaac19ac6b87a687a841d01f755ee4f49d08cc6cb0188ada2b30fa837663756d",
    ),
    (
        5,
        "Items",
        "score-null",
        "h5-random-score-null.accdb",
        "c65c24b52a0cb14fdc93e3f655a285cd809f95397803ab920899eb5e183906d1",
    ),
    (
        6,
        "Items",
        "before",
        "h6-random-before.accdb",
        "8d729549fed474fdd3a7bf3731254ec46f100275e480993614ecf78e531dad2e",
    ),
    (
        6,
        "Items",
        "change",
        "h6-random-change.accdb",
        "c91f497594e8e17d593901169b57c1c842f07a2c7b9069af618bd41b166de1be",
    ),
    (
        6,
        "Items",
        "score-null",
        "h6-random-score-null.accdb",
        "d5d3e206277cd927e50fdbb79311756cd9aa7119c8aa657b58fcbd172c0e55bf",
    ),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn rows(table: &str, recipe: &str) -> Vec<Vec<Value>> {
    let mut rows = if table == "Numbered" {
        vec![
            vec![Value::Int32(101), Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(102), Value::Int32(2), Value::Int32(20)],
        ]
    } else {
        assert_eq!(table, "Items");
        vec![vec![Value::Int32(1), Value::Int32(10)]]
    };
    let score = if table == "Numbered" { 2 } else { 1 };
    match recipe {
        "before" => {}
        "batch" => {
            assert_eq!(table, "Numbered");
            rows[0][score] = Value::Int32(17);
            rows[1][score] = Value::Int32(23);
        }
        "change" => {
            assert_eq!(table, "Items");
            rows[0][score] = Value::Int32(17);
        }
        "score-null" => {
            let ordinal = usize::from(table == "Numbered");
            rows[ordinal][score] = Value::Null;
        }
        _ => panic!("only literal source roles"),
    }
    rows
}

#[test]
#[ignore = "requires twelve independently modeled generated-companion Score views"]
fn generated_companion_score_sources_preserve_counters_defaults_and_full_canonical_source() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_GENERATED_COMPANION_UPDATE_SOURCE_DIR").unwrap(),
    );
    assert!(directory.is_absolute());
    assert_eq!(
        digest(&fs::read(directory.join("manifest.json")).unwrap()),
        MANIFEST_SHA
    );
    assert_eq!(VIEWS.len(), 12);
    let views = VIEWS
        .iter()
        .map(|&(header, table, recipe, filename, hash)| {
            let raw = fs::read(directory.join(filename)).unwrap();
            assert_eq!(digest(&raw), hash);
            (header, table, recipe, raw)
        })
        .collect::<Vec<_>>();

    for (header, table, recipe, raw) in &views {
        let before = &views
            .iter()
            .find(|item| item.0 == *header && item.1 == *table && item.2 == "before")
            .unwrap()
            .3;
        let numbered = *table == "Numbered";
        let definition = if numbered {
            if *header == 5 { 94 } else { 111 }
        } else {
            7
        };
        let data = if numbered {
            if *header == 5 { 97 } else { 115 }
        } else {
            8
        };
        let root = if numbered {
            if *header == 5 { 96 } else { 113 }
        } else {
            9
        };
        let generated = usize::from(numbered);
        let last = if numbered { 3_i32 } else { 137 };
        let history = if numbered { 3_u32 } else { 1 };
        let aliases: &[&str] = if numbered {
            &["PrimaryID", "PrimaryAlias"]
        } else {
            &["PrimaryId"]
        };
        let expected_rows = rows(table, recipe);
        let mut expected = before.clone();
        let base = data * 4096;
        match *recipe {
            "before" => {}
            "batch" => {
                expected[base + 4091..base + 4095].copy_from_slice(&17_i32.to_le_bytes());
                expected[base + 4076..base + 4080].copy_from_slice(&23_i32.to_le_bytes());
            }
            "change" => expected[base + 4091..base + 4095].copy_from_slice(&17_i32.to_le_bytes()),
            "score-null" => {
                if numbered {
                    assert_eq!(&before[base + 4076..base + 4080], &20_i32.to_le_bytes());
                    expected[base + 4080] = 3;
                } else {
                    assert_eq!(&before[base + 4091..base + 4095], &10_i32.to_le_bytes());
                    expected[base + 4095] = 1;
                }
            }
            _ => panic!("closed literal request law"),
        }
        assert_eq!(raw, &expected);
        assert_eq!(&raw[20..24], &header.to_le_bytes());
        assert_eq!(
            &raw[definition * 4096..(definition + 1) * 4096],
            &before[definition * 4096..(definition + 1) * 4096]
        );
        assert_eq!(
            &raw[root * 4096..(root + 1) * 4096],
            &before[root * 4096..(root + 1) * 4096]
        );
        assert_eq!(
            &raw[definition * 4096 + 20..definition * 4096 + 24],
            &last.to_le_bytes()
        );
        assert_eq!(
            &raw[definition * 4096 + 67..definition * 4096 + 71],
            &history.to_le_bytes()
        );
        if numbered {
            assert_eq!(&raw[base + 18..base + 20], &0xcfd3_u16.to_le_bytes());
            assert_eq!(&raw[base + 2..base + 4], &4031_u16.to_le_bytes());
            assert_eq!(&raw[base + 12..base + 14], &3_u16.to_le_bytes());
            assert_eq!(
                &raw[base + 4051..base + 4066],
                &before[base + 4051..base + 4066]
            );
        }

        let image =
            NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
        let snapshot =
            read_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
        assert_eq!(snapshot.rows(), expected_rows);
        let schema = snapshot.schema();
        assert_eq!(schema.definition_page() as usize, definition);
        for (position, field) in schema.columns().iter().enumerate() {
            assert_eq!(
                (
                    field.number(),
                    field.native_type(),
                    field.storage_length(),
                    field.extended_flags()
                ),
                (position, 4, 4, 0)
            );
            assert_eq!(field.flags(), if position == generated { 7 } else { 3 });
        }
        let properties =
            inspect_table_properties(&mut image.reader(), SCOPE, table, &mut budget(), &|| false)
                .unwrap();
        if numbered {
            assert!(properties.raw_bytes().is_empty() && properties.groups().is_empty());
        } else {
            crate::properties::framing::validate(&properties, &mut budget(), &|| false).unwrap();
            assert_eq!(properties.raw_bytes().len(), 86);
            let [group] = properties.groups() else {
                panic!("exact preserved Random declaration")
            };
            assert_eq!(group.kind(), PropertyGroupKind::Column);
            assert_eq!(group.name(), "Id");
            assert_eq!(group.properties().len(), 1);
            let property = group.get("DefaultValue").unwrap();
            assert_eq!((property.native_type(), property.flags()), (12, 1));
            assert_eq!(property.text().unwrap(), "GenUniqueID()");
        }
        let checked =
            check_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
        assert_eq!(checked.rows(), expected_rows.len());
        assert_eq!(checked.indexes().len(), aliases.len());
        for index in checked.indexes() {
            assert_eq!(index.status(), IndexCheckStatus::Checked);
            assert_eq!(index.index().root_page() as usize, root);
        }
        for alias in aliases {
            assert_eq!(
                read_table_index(
                    &mut image.reader(),
                    SCOPE,
                    table,
                    alias,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                expected_rows
            );
            assert_eq!(
                inspect_table_index_counter(
                    &mut image.reader(),
                    SCOPE,
                    table,
                    alias,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .value(),
                history
            );
        }
        let constraints =
            check_table_constraints(&mut image.reader(), SCOPE, table, &mut budget(), &|| false)
                .unwrap();
        assert_eq!(constraints.rows(), expected_rows.len());
        assert_eq!(constraints.table_validation(), RuleCheckStatus::NotDeclared);
        assert!(constraints.relationships().is_empty());
        for field in constraints.fields() {
            assert_eq!(field.required(), RuleCheckStatus::NotDeclared);
            assert_eq!(field.empty_text(), RuleCheckStatus::NotDeclared);
            assert_eq!(field.validation(), RuleCheckStatus::NotDeclared);
        }
        let captured = super::capture(&image, table, &mut budget(), &|| false).unwrap();
        assert_eq!(captured.table.rows(), expected_rows);
        assert_eq!(captured.records.len(), expected_rows.len());
        image
            .check_native_ordinary_fixed_table(table, &mut budget(), &|| false)
            .unwrap();
        assert!(matches!(
            crate::table::mutation::field::fixed_column_at(schema, generated, ScalarType::Int32, 4),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
        assert_eq!(image.as_bytes(), raw);
    }
}
