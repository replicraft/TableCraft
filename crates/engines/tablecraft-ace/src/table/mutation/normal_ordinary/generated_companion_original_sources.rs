//! Observe existing canonical UPDATE admission; no AFTER, staging or generation.
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, IndexCheckStatus, NativeImage,
    PropertyGroupKind, RuleCheckStatus, WriteBlocker, check_table, check_table_constraints,
    inspect_table_index_counter, inspect_table_properties, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const SCOPE: CatalogScope = CatalogScope::new(241);
const RANDOM_DEFAULT: &str = "GenUniqueID()";

struct Original {
    header: u32,
    family: &'static str,
    directory: &'static str,
    filename: &'static str,
    length: usize,
    sha256: &'static str,
    table: &'static str,
    definition: usize,
    data: usize,
    root: u32,
    generated: usize,
    last: i32,
    history: u32,
    aliases: &'static [&'static str],
}

const ORIGINALS: [Original; 4] = [
    Original {
        header: 5,
        family: "genuine-middle-generated",
        directory: "TABLECRAFT_GENERATED_COMPANION_GENUINE_DIR",
        filename: "h5-default-original.accdb",
        length: 446464,
        sha256: "c6dc5d36f35335ea2d9cafbc5cf6500dc135ea41a4d3f60d9241ac9e55714d12",
        table: "Numbered",
        definition: 94,
        data: 97,
        root: 96,
        generated: 1,
        last: 3,
        history: 3,
        aliases: &["PrimaryID", "PrimaryAlias"],
    },
    Original {
        header: 6,
        family: "genuine-middle-generated",
        directory: "TABLECRAFT_GENERATED_COMPANION_GENUINE_DIR",
        filename: "h6-default-original.accdb",
        length: 520192,
        sha256: "c6aae0208a2dfbd0b77f8867f49700718880ee761c7881df6ec5789843a8b456",
        table: "Numbered",
        definition: 111,
        data: 115,
        root: 113,
        generated: 1,
        last: 3,
        history: 3,
        aliases: &["PrimaryID", "PrimaryAlias"],
    },
    Original {
        header: 5,
        family: "preserved-random-declaration",
        directory: "TABLECRAFT_GENERATED_COMPANION_RANDOM_DIR",
        filename: "h5-random-special-default.accdb",
        length: 40960,
        sha256: "d45a6157d94c892b8963449796b30d5e54512de1894fa9def47c042b240c2638",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated: 0,
        last: 137,
        history: 1,
        aliases: &["PrimaryId"],
    },
    Original {
        header: 6,
        family: "preserved-random-declaration",
        directory: "TABLECRAFT_GENERATED_COMPANION_RANDOM_DIR",
        filename: "h6-random-special-default.accdb",
        length: 40960,
        sha256: "8d729549fed474fdd3a7bf3731254ec46f100275e480993614ecf78e531dad2e",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated: 0,
        last: 137,
        history: 1,
        aliases: &["PrimaryId"],
    },
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn observe<T>(case: &Original, operation: &str, result: &Result<T, EditError>) -> bool {
    match result {
        Ok(_) => {
            println!(
                "GENERATED_COMPANION_UPDATE_PROBE\t{}\t{}\t{operation}\tADMITTED",
                case.header, case.family
            );
            true
        }
        Err(error) => {
            // Native errors contain typed, sanitized causes; no row/property
            // payload or stored expression is emitted by this observation.
            println!(
                "GENERATED_COMPANION_UPDATE_PROBE\t{}\t{}\t{operation}\tREFUSED:{error:?}",
                case.header, case.family
            );
            false
        }
    }
}

fn expected_rows(case: &Original) -> Vec<Vec<Value>> {
    if case.generated == 1 {
        vec![
            vec![Value::Int32(101), Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(102), Value::Int32(2), Value::Int32(20)],
        ]
    } else {
        vec![vec![Value::Int32(1), Value::Int32(10)]]
    }
}

fn original_records(case: &Original, raw: &[u8]) {
    let base = case.data * 4096;
    if case.generated == 1 {
        assert_eq!(&raw[base + 2..base + 4], &4031_u16.to_le_bytes());
        assert_eq!(&raw[base + 12..base + 14], &3_u16.to_le_bytes());
        for (slot, word) in [0x0ff1_u16, 0x0fe2, 0xcfd3].into_iter().enumerate() {
            assert_eq!(
                &raw[base + 14 + 2 * slot..base + 16 + 2 * slot],
                &word.to_le_bytes()
            );
        }
        assert_eq!(
            &raw[base + 4081..base + 4096],
            &[3, 0, 101, 0, 0, 0, 1, 0, 0, 0, 10, 0, 0, 0, 7]
        );
        assert_eq!(
            &raw[base + 4066..base + 4081],
            &[3, 0, 102, 0, 0, 0, 2, 0, 0, 0, 20, 0, 0, 0, 7]
        );
        assert_eq!(
            &raw[case.definition * 4096 + 16..case.definition * 4096 + 20],
            &2_u32.to_le_bytes()
        );
    } else {
        assert_eq!(&raw[base + 12..base + 14], &1_u16.to_le_bytes());
        assert_eq!(&raw[base + 14..base + 16], &4085_u16.to_le_bytes());
        assert_eq!(
            &raw[base + 4085..base + 4096],
            &[2, 0, 1, 0, 0, 0, 10, 0, 0, 0, 3]
        );
        assert_eq!(
            &raw[case.definition * 4096 + 16..case.definition * 4096 + 20],
            &1_u32.to_le_bytes()
        );
    }
    assert_eq!(
        &raw[case.definition * 4096 + 20..case.definition * 4096 + 24],
        &case.last.to_le_bytes()
    );
    assert_eq!(
        &raw[case.definition * 4096 + 67..case.definition * 4096 + 71],
        &case.history.to_le_bytes()
    );
}

#[test]
#[ignore = "requires exactly four pinned existing generated-companion originals; observations only"]
fn existing_generated_companion_originals_report_canonical_source_and_fixed_capability_without_staging()
 {
    // All four exact original pins precede the first native capture. No copied
    // AFTER, generator, resolver or mutation planner is an input to this probe.
    let originals = ORIGINALS
        .iter()
        .map(|case| {
            let directory = PathBuf::from(std::env::var_os(case.directory).unwrap());
            assert!(directory.is_absolute());
            let raw = fs::read(directory.join(case.filename)).unwrap();
            assert_eq!(raw.len(), case.length);
            let hash: String = Sha256::digest(&raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(hash, case.sha256);
            (case, raw)
        })
        .collect::<Vec<_>>();

    let mut source_admitted = 0;
    let mut capability_admitted = 0;
    for (case, raw) in &originals {
        assert_eq!(&raw[20..24], &case.header.to_le_bytes());
        original_records(case, raw);
        let image =
            NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
        let table = read_table(
            &mut image.reader(),
            SCOPE,
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let expected = expected_rows(case);
        assert_eq!(table.rows(), expected);
        let schema = table.schema();
        assert_eq!(schema.definition_page() as usize, case.definition);
        assert_eq!(
            schema.columns().len(),
            if case.generated == 1 { 3 } else { 2 }
        );
        for (position, field) in schema.columns().iter().enumerate() {
            assert_eq!(field.number(), position);
            assert_eq!(field.native_type(), 4);
            assert_eq!(field.storage_length(), 4);
            assert_eq!(field.field.offset, 4 * position);
            assert_eq!(
                field.flags(),
                if position == case.generated { 7 } else { 3 }
            );
            assert_eq!(field.extended_flags(), 0);
        }

        let properties = inspect_table_properties(
            &mut image.reader(),
            SCOPE,
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        if case.generated == 1 {
            assert!(properties.raw_bytes().is_empty());
            assert!(properties.groups().is_empty());
        } else {
            crate::properties::framing::validate(&properties, &mut budget(), &|| false).unwrap();
            assert_eq!(properties.raw_bytes().len(), 86);
            let [group] = properties.groups() else {
                panic!("exact original Random declaration group")
            };
            assert_eq!(group.kind(), PropertyGroupKind::Column);
            assert_eq!(group.name(), "Id");
            assert_eq!(group.properties().len(), 1);
            let property = group.get("DefaultValue").unwrap();
            assert_eq!((property.native_type(), property.flags()), (12, 1));
            assert_eq!(property.text().unwrap(), RANDOM_DEFAULT);
            assert_eq!(
                property.raw_value(),
                RANDOM_DEFAULT
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>()
            );
        }

        let checked = check_table(
            &mut image.reader(),
            SCOPE,
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), expected.len());
        assert_eq!(checked.indexes().len(), case.aliases.len());
        for index in checked.indexes() {
            assert_eq!(index.status(), IndexCheckStatus::Checked);
            assert_eq!(index.index().root_page(), case.root);
            let [key] = index.index().columns() else {
                panic!("exact original one-field primary")
            };
            assert_eq!(key.number(), case.generated);
            assert!(!key.descending());
        }
        for alias in case.aliases {
            assert_eq!(
                read_table_index(
                    &mut image.reader(),
                    SCOPE,
                    case.table,
                    alias,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                expected
            );
            assert_eq!(
                inspect_table_index_counter(
                    &mut image.reader(),
                    SCOPE,
                    case.table,
                    alias,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .value(),
                case.history
            );
        }
        let constraints = check_table_constraints(
            &mut image.reader(),
            SCOPE,
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(constraints.rows(), expected.len());
        assert_eq!(constraints.table_validation(), RuleCheckStatus::NotDeclared);
        assert!(constraints.relationships().is_empty());
        for field in constraints.fields() {
            assert_eq!(field.required(), RuleCheckStatus::NotDeclared);
            assert_eq!(field.empty_text(), RuleCheckStatus::NotDeclared);
            assert_eq!(field.validation(), RuleCheckStatus::NotDeclared);
        }

        let guard = crate::table::mutation::field::fixed_column_at(
            schema,
            case.generated,
            ScalarType::Int32,
            4,
        );
        observe(case, "generated_field_guard", &guard);
        assert!(matches!(
            guard,
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
        let captured = super::capture(&image, case.table, &mut budget(), &|| false);
        source_admitted += usize::from(observe(case, "canonical_source", &captured));
        if let Ok(source) = &captured {
            assert_eq!(source.table.rows(), expected);
            assert_eq!(source.records.len(), expected.len());
            let empty_score = [FixedColumnUpdates::new(case.table, "Score", &[])];
            let score = super::preflight(source, &empty_score, &mut budget(), &|| false);
            observe(case, "ordinary_score_empty_binding", &score);
            for replacement in [Some(4), Some(1), None] {
                let updates = replacement
                    .map(|value| FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(value)))
                    .into_iter()
                    .collect::<Vec<_>>();
                let groups = [FixedColumnUpdates::new(case.table, "Id", &updates)];
                let generated = super::preflight(source, &groups, &mut budget(), &|| false);
                let label = match replacement {
                    Some(4) => "generated_changed_binding",
                    Some(_) => "generated_noop_binding",
                    None => "generated_empty_binding",
                };
                observe(case, label, &generated);
                assert!(matches!(
                    generated,
                    Err(EditError::Unsupported(WriteBlocker::Column))
                ));
            }
        }
        let capability =
            image.check_native_ordinary_fixed_table(case.table, &mut budget(), &|| false);
        capability_admitted += usize::from(observe(case, "fixed_capability", &capability));
        assert_eq!(image.as_bytes(), raw);
    }
    for (case, raw) in &originals {
        let directory = PathBuf::from(std::env::var_os(case.directory).unwrap());
        assert_eq!(fs::read(directory.join(case.filename)).unwrap(), *raw);
    }
    assert_eq!(originals.len(), 4);
    println!(
        "GENERATED_COMPANION_UPDATE_PROBE_SUMMARY\t4\t{source_admitted}\t{capability_admitted}"
    );
}
