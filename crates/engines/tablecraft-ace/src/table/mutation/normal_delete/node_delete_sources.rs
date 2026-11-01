//! Four literal source views; no normal planner or preview candidate is invoked.
use crate::{
    IndexCheckStatus, NativeImage, inspect_table_index_counter, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(175);

struct Case {
    definition: u32,
    roots: [u32; 2],
    children: [[u32; 2]; 2],
    data: u32,
    before: Vec<u8>,
    after: Vec<u8>,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_RETAINED_NODE_DELETE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut result = Vec::new();
    for (header, length, before_sha, after_sha, definition, roots, children, data) in [
        (
            5,
            434176,
            "0d9cf91fff6c9153c628bf5cd54dc39a9420b18f4fad669fca8e9a3c8a92694b",
            "265e86c0aa1deded6031e2d367883044401f6647bf058aee18c075ffccc889b9",
            89,
            [91, 92],
            [[98, 97], [101, 100]],
            95,
        ),
        (
            6,
            503808,
            "18eff74333ed55f4bfef879d6a2530fab8031aaef6a07895f50aec13fdf079ae",
            "23804bc0cc113d6fe2d8279d88329fd1f2aa80852fbf9c32638296b13dd3fdcd",
            106,
            [108, 109],
            [[115, 114], [118, 117]],
            112,
        ),
    ] {
        let load = |suffix: &str, wanted: &str| {
            let bytes =
                fs::read(directory.join(format!("h{header}-retained-node-delete-{suffix}.accdb")))
                    .unwrap();
            assert_eq!(bytes.len(), length);
            let digest = Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(digest, wanted);
            bytes
        };
        let before = load("before", before_sha);
        let after = load("after", after_sha);
        result.push(Case {
            definition,
            roots,
            children,
            data,
            before,
            after,
        });
    }
    result
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap()
}

fn rows(deleted: bool) -> Vec<Vec<Value>> {
    (0..800)
        .filter(|position| !deleted || ![301, 353].contains(position))
        .map(|position| {
            vec![
                Value::Int32(position + 1),
                Value::Int32(position - 400),
                Value::Int32(position % 7),
            ]
        })
        .collect()
}

#[test]
#[ignore = "requires all four literal retained-NODE DELETE views after SDK24 closure"]
fn retained_node_delete_sources_preserve_owned_topology_history_and_full_fences() {
    let mut views = 0;
    for case in cases() {
        let original = image(&case.before);
        let original_source =
            super::super::normal_ordinary::capture(&original, "Items", &mut budget(), &|| false)
                .unwrap();
        assert_eq!(original_source.table.rows(), rows(false));
        for (raw, deleted) in [(&case.before, false), (&case.after, true)] {
            let image = image(raw);
            let source =
                super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| false)
                    .unwrap();
            let expected = rows(deleted);
            assert_eq!(source.table.schema().definition_page(), case.definition);
            assert_eq!(source.records.len(), if deleted { 798 } else { 800 });
            assert_eq!(source.table.rows(), expected);
            let checked = crate::table::indexes::check_table_retained(
                &mut image.reader(),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(checked.rows(), expected.len());
            assert_eq!(checked.indexes().len(), 4);
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            let snapshot = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(snapshot.rows(), expected);

            let mut guard = original_source
                .records
                .iter()
                .zip(original_source.table.rows())
                .filter(|(_, row)| !deleted || !matches!(row[0], Value::Int32(302 | 354)))
                .map(|(record, row)| {
                    let Value::Int32(value) = row[2] else {
                        panic!("literal Guard");
                    };
                    (value, record.reference, row.clone())
                })
                .collect::<Vec<_>>();
            guard.sort_unstable_by_key(|(value, reference, _)| (*value, *reference));
            let guard = guard.into_iter().map(|(_, _, row)| row).collect::<Vec<_>>();
            for (name, physical, wanted, historical) in [
                ("PrimaryID", 0, expected.clone(), 800),
                ("PrimaryAlias", 0, expected.clone(), 800),
                ("GroupOrder", 1, guard.clone(), 7),
                ("GroupAlias", 1, guard, 7),
            ] {
                let selected = checked
                    .indexes()
                    .iter()
                    .find(|entry| entry.index().name() == name)
                    .unwrap()
                    .index();
                assert_eq!(selected.root_page(), case.roots[physical]);
                assert_eq!(selected.data_number() as usize, physical);
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
                let count = inspect_table_index_counter(
                    &mut image.reader(),
                    SCOPE,
                    "Items",
                    name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(count.value(), historical);
            }
            for physical in 0..2 {
                let base = case.roots[physical] as usize * 4096;
                let page = &raw[base..base + 4096];
                assert_eq!(&page[..2], &[3, 1]);
                let (value, slot) = if physical == 0 {
                    (
                        if deleted { 301_i32 } else { 302 },
                        if deleted { 60 } else { 61 },
                    )
                } else {
                    (3_i32, if deleted { 106 } else { 113 })
                };
                assert_eq!(page[480], 0x7f);
                assert_eq!(
                    &page[481..485],
                    &((value as u32) ^ 0x8000_0000).to_be_bytes()
                );
                assert_eq!(&page[485..489], &((case.data << 8) | slot).to_be_bytes());
                assert_eq!(&page[489..493], &case.children[physical][0].to_be_bytes());
                assert_eq!(&page[20..24], &case.children[physical][1].to_le_bytes());
            }
            assert_eq!(image.as_bytes(), raw);
            views += 1;
        }
        let td = case.definition as usize * 4096;
        assert_eq!(
            &case.before[td + 67..td + 83],
            &case.after[td + 67..td + 83]
        );
        for slot in [61, 113] {
            let at = case.data as usize * 4096 + 14 + 2 * slot;
            let word = u16::from_le_bytes(case.before[at..at + 2].try_into().unwrap());
            assert_eq!(&case.after[at..at + 2], &(word | 0xc000).to_le_bytes());
        }
    }
    assert_eq!(views, 4);
}
