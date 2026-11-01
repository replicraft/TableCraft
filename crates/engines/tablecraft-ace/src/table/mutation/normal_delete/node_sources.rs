//! Original NODE qualification and byte-exact empty-operation admission.
use crate::{IndexCheckStatus, NativeImage, read_table, read_table_index};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(174);

struct Case {
    before: Vec<u8>,
    definition: u32,
    roots: [u32; 2],
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_NODE_ORIGINAL_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, length, digest, definition, roots) in [
        (
            5,
            434176,
            "0d9cf91fff6c9153c628bf5cd54dc39a9420b18f4fad669fca8e9a3c8a92694b",
            89,
            [91, 92],
        ),
        (
            6,
            503808,
            "18eff74333ed55f4bfef879d6a2530fab8031aaef6a07895f50aec13fdf079ae",
            106,
            [108, 109],
        ),
    ] {
        let before =
            fs::read(directory.join(format!("h{header}-node-index-original.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        let actual = Sha256::digest(&before)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, digest);
        assert_eq!(before[20], header);
        cases.push(Case {
            before,
            definition,
            roots,
        });
    }
    cases
}

fn image(case: &Case) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(&case.before),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn literal_rows() -> Vec<Vec<Value>> {
    (0..800)
        .map(|position| {
            vec![
                Value::Int32(position + 1),
                Value::Int32(position - 400),
                Value::Int32(position % 7),
            ]
        })
        .collect()
}

fn qualify(case: &Case) {
    let image = image(case);
    let source =
        super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    assert_eq!(source.table.schema().definition_page(), case.definition);
    assert_eq!(source.records.len(), 800);
    assert_eq!(source.table.rows(), literal_rows());
    assert_eq!(source.indexes.indexes().len(), 4);

    let checked = crate::table::indexes::check_table_retained(
        &mut image.reader(),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 800);
    assert_eq!(checked.indexes().len(), 4);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    for root in case.roots {
        let page = &case.before[root as usize * 4096..(root as usize + 1) * 4096];
        assert_eq!(&page[..2], &[3, 1]);
        assert_eq!(&page[4..8], &case.definition.to_le_bytes());
    }

    let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(table.rows(), literal_rows());
    let mut guard = source
        .records
        .iter()
        .zip(source.table.rows())
        .map(|(record, row)| {
            let Value::Int32(value) = row[2] else {
                panic!("literal Guard carrier");
            };
            (value, record.reference, row.clone())
        })
        .collect::<Vec<_>>();
    guard.sort_unstable_by_key(|(value, reference, _)| (*value, *reference));
    let guard = guard.into_iter().map(|(_, _, row)| row).collect::<Vec<_>>();
    for (name, physical, wanted) in [
        ("PrimaryID", 0, literal_rows()),
        ("PrimaryAlias", 0, literal_rows()),
        ("GroupOrder", 1, guard.clone()),
        ("GroupAlias", 1, guard),
    ] {
        let declaration = checked
            .indexes()
            .iter()
            .find(|entry| entry.index().name() == name)
            .unwrap();
        assert_eq!(declaration.index().data_number() as usize, physical);
        assert_eq!(declaration.index().root_page(), case.roots[physical]);
        let cursor = read_table_index(
            &mut image.reader(),
            SCOPE,
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(cursor.rows(), wanted);
    }
    assert_eq!(image.as_bytes(), case.before);
}

#[test]
#[ignore = "requires both hash-pinned SDK NODE originals after twelve readonly observations"]
fn sdk_node_originals_have_complete_native_source_rows_aliases_and_owned_tree_membership() {
    for case in cases() {
        qualify(&case);
    }
}

#[test]
#[ignore = "requires the separate native NODE source qualification gate to pass first"]
fn healthy_node_empty_insert_preserves_every_original_byte_after_qualification() {
    for case in cases() {
        qualify(&case);
        let plan = image(&case)
            .plan_recoverable_ordinary_row_inserts("Items", vec![], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.before);
        assert_eq!(plan.inserted_rows(), 0);
        assert!(plan.intent().rows().is_empty());
    }
}
