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
    fence_data: u32,
    target_data: u32,
    before: Vec<u8>,
    after: Vec<u8>,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_NODE_INSERT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut result = Vec::new();
    for (
        header,
        length,
        before_sha,
        after_sha,
        definition,
        roots,
        children,
        fence_data,
        target_data,
    ) in [
        (
            5,
            434176,
            "0d9cf91fff6c9153c628bf5cd54dc39a9420b18f4fad669fca8e9a3c8a92694b",
            "38f15336e6810158e6fd6b67b5cf1d12ab1c4869fc7d233becc13a12097e1320",
            89,
            [91, 92],
            [[98, 97], [101, 100]],
            95,
            99,
        ),
        (
            6,
            503808,
            "18eff74333ed55f4bfef879d6a2530fab8031aaef6a07895f50aec13fdf079ae",
            "d71fe690d393d1f0a5f5d2256976b452e408cc93cbbbf760fe46c83f2a97144a",
            106,
            [108, 109],
            [[115, 114], [118, 117]],
            112,
            116,
        ),
    ] {
        let load = |suffix: &str, wanted: &str| {
            let bytes =
                fs::read(directory.join(format!("h{header}-retained-node-insert-{suffix}.accdb")))
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
            fence_data,
            target_data,
            before,
            after,
        });
    }
    result
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap()
}

fn rows(inserted: bool) -> Vec<Vec<Value>> {
    let mut result = (0..800)
        .map(|position| {
            vec![
                Value::Int32(position + 1),
                Value::Int32(position - 400),
                Value::Int32(position % 7),
            ]
        })
        .collect::<Vec<_>>();
    if inserted {
        result.push(vec![Value::Int32(801), Value::Int32(400), Value::Int32(1)]);
    }
    result
}

#[test]
#[ignore = "requires all four literal fitting NODE INSERT views after SDK24 closure"]
fn node_insert_sources_preserve_owned_topology_and_explicit_population_partitions() {
    let mut views = 0;
    for case in cases() {
        let original = image(&case.before);
        let original_source =
            super::super::normal_ordinary::capture(&original, "Items", &mut budget(), &|| false)
                .unwrap();
        assert_eq!(original_source.table.rows(), rows(false));
        for (raw, inserted) in [(&case.before, false), (&case.after, true)] {
            let image = image(raw);
            let source =
                super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| false)
                    .unwrap();
            let expected = rows(inserted);
            assert_eq!(source.table.schema().definition_page(), case.definition);
            assert_eq!(source.records.len(), if inserted { 801 } else { 800 });
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
                .map(|(record, row)| {
                    let Value::Int32(value) = row[2] else {
                        panic!("literal Guard");
                    };
                    (value, record.reference, row.clone())
                })
                .collect::<Vec<_>>();
            if inserted {
                guard.push((
                    1,
                    (case.target_data << 8) | 80,
                    vec![Value::Int32(801), Value::Int32(400), Value::Int32(1)],
                ));
            }
            guard.sort_unstable_by_key(|(value, reference, _)| (*value, *reference));
            let guard = guard.into_iter().map(|(_, _, row)| row).collect::<Vec<_>>();
            for (name, physical, wanted, historical) in [
                (
                    "PrimaryID",
                    0,
                    expected.clone(),
                    if inserted { 801 } else { 800 },
                ),
                (
                    "PrimaryAlias",
                    0,
                    expected.clone(),
                    if inserted { 801 } else { 800 },
                ),
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
                    (302_i32, 61)
                } else {
                    (3_i32, if inserted { 106 } else { 113 })
                };
                assert_eq!(page[480], 0x7f);
                assert_eq!(
                    &page[481..485],
                    &((value as u32) ^ 0x8000_0000).to_be_bytes()
                );
                assert_eq!(
                    &page[485..489],
                    &((case.fence_data << 8) | slot).to_be_bytes()
                );
                assert_eq!(&page[489..493], &case.children[physical][0].to_be_bytes());
                assert_eq!(&page[20..24], &case.children[physical][1].to_le_bytes());
            }
            assert_eq!(image.as_bytes(), raw);
            views += 1;
        }
        let td = case.definition as usize * 4096;
        assert_eq!(
            &case.before[td + 20..td + 24],
            &case.after[td + 20..td + 24]
        );
        assert_eq!(&case.after[td + 67..td + 71], &801_u32.to_le_bytes());
        assert_eq!(
            &case.before[td + 79..td + 83],
            &case.after[td + 79..td + 83]
        );
        let base = case.target_data as usize * 4096;
        assert_eq!(&case.before[base + 12..base + 14], &80_u16.to_le_bytes());
        assert_eq!(&case.after[base + 12..base + 14], &81_u16.to_le_bytes());
        assert_eq!(&case.after[base + 174..base + 176], &2881_u16.to_le_bytes());
        assert_eq!(&case.after[base + 2..base + 4], &2705_u16.to_le_bytes());
    }
    assert_eq!(views, 4);
}
