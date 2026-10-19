use super::{budget, capture, index_fixture, tree_fixture};
use proptest::prelude::*;
use std::io::Cursor;
use tablecraft_ace::{Int32Update, check_table, read_table_index};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const DEFINITION: usize = 10;
const PHYSICAL_FLAGS: usize = 189;
const LOGICAL_START: usize = 195;
const LOGICAL_BYTES: usize = 28;
const INDEX_TYPE: usize = 23;
const ENTRY_START: usize = 480;

fn database(descending: bool, alias: bool, nullable: bool, topology: usize) -> Vec<u8> {
    let mut bytes = if topology == 0 {
        index_fixture::database(descending, alias, nullable)
    } else {
        tree_fixture::database(descending, nullable, topology == 2)
    };
    let metadata = index_fixture::database(descending, alias, nullable);
    bytes[DEFINITION * PAGE_BYTES..(DEFINITION + 1) * PAGE_BYTES]
        .copy_from_slice(&metadata[DEFINITION * PAGE_BYTES..(DEFINITION + 1) * PAGE_BYTES]);
    bytes[DEFINITION * PAGE_BYTES + PHYSICAL_FLAGS] = 0x80;
    for index in 0..if alias { 2 } else { 1 } {
        bytes[DEFINITION * PAGE_BYTES + LOGICAL_START + index * LOGICAL_BYTES + INDEX_TYPE] = 0;
    }
    bytes
}
fn verify(before: &[u8], after: &[u8], descending: bool, nullable: bool, updates: &[Int32Update]) {
    let mut expected = vec![
        (if nullable { None } else { Some(10) }, 1),
        (Some(20), 2),
        (Some(30), 3),
    ];
    for update in updates {
        expected[update.row()].0 = Some(update.value());
    }
    expected.sort_by(|a, b| {
        let key = a.0.cmp(&b.0);
        (if descending { key.reverse() } else { key }).then(a.1.cmp(&b.1))
    });
    let indexed = read_table_index(
        &mut Cursor::new(after),
        CatalogScope::new(1),
        "Items",
        "ScoreKey",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let actual = indexed
        .rows()
        .iter()
        .map(|row| {
            let key = match row[1] {
                Value::Null => None,
                Value::Int32(value) => Some(value),
                _ => panic!("key"),
            };
            let Value::Int32(id) = row[0] else {
                panic!("row identity")
            };
            (key, id)
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    let report = check_table(
        &mut Cursor::new(after),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        report
            .indexes()
            .iter()
            .all(|i| i.status() == tablecraft_ace::IndexCheckStatus::Checked
                && !i.index().constraints().unwrap().unique())
    );
    assert_eq!(before.len(), after.len());
    let mut allowed = vec![false; before.len()];
    for update in updates {
        let slot = 12 * PAGE_BYTES + 14 + update.row() * 2;
        let row = usize::from(u16::from_le_bytes(
            before[slot..slot + 2].try_into().unwrap(),
        )) & 0x1fff;
        allowed[12 * PAGE_BYTES + row + 6..12 * PAGE_BYTES + row + 10].fill(true);
    }
    for (page, bytes) in before.as_chunks::<PAGE_BYTES>().0.iter().enumerate() {
        if matches!(bytes[0], 3 | 4)
            && u32::from_le_bytes(bytes[4..8].try_into().unwrap()) == DEFINITION as u32
        {
            let end = PAGE_BYTES - usize::from(u16::from_le_bytes(bytes[2..4].try_into().unwrap()));
            allowed[page * PAGE_BYTES + ENTRY_START..page * PAGE_BYTES + end].fill(true);
        }
    }
    assert!(
        before
            .iter()
            .zip(after)
            .zip(allowed)
            .all(|((&a, &b), allowed)| a == b || allowed)
    );
}

#[test]
fn ordinary_duplicate_keys_keep_row_ties_ascending_across_leaf_node_deep_and_alias_layouts() {
    for descending in [false, true] {
        for nullable in [false, true] {
            for alias in [false, true] {
                for topology in 0..3 {
                    let bytes = database(descending, alias, nullable, topology);
                    let updates = [Int32Update::new(1, 20, 30)];
                    let plan = capture(&bytes)
                        .plan_int32_updates("Items", "Score", &updates, &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(plan.changed_rows(), 1);
                    assert_eq!(plan.before_bytes(), bytes);
                    verify(&bytes, plan.after_bytes(), descending, nullable, &updates);
                    let plan = capture(plan.after_bytes())
                        .plan_int32_updates(
                            "Items",
                            "Score",
                            &[Int32Update::new(1, 30, 20)],
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                    assert_eq!(plan.after_bytes(), bytes);
                }
            }
        }
    }
}

#[test]
fn duplicate_batches_noops_and_deduplicated_alias_patches_preserve_original_images() {
    for descending in [false, true] {
        let bytes = database(descending, true, false, 2);
        let updates = [Int32Update::new(0, 10, 30), Int32Update::new(1, 20, 30)];
        let plan = capture(&bytes)
            .plan_int32_updates("Items", "Score", &updates, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.changed_rows(), 2);
        verify(&bytes, plan.after_bytes(), descending, false, &updates);
        let plan = capture(&bytes)
            .plan_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(1, 20, 20)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.changed_rows(), 0);
        assert_eq!(plan.after_bytes(), bytes);
        let compressed = database(descending, false, false, 0);
        assert_eq!(
            capture(&compressed)
                .plan_int32_updates(
                    "Items",
                    "Score",
                    &[Int32Update::new(1, 20, i32::MAX)],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(tablecraft_ace::EditError::Unsupported(
                tablecraft_ace::WriteBlocker::IndexedColumn
            ))
        );
    }
}

#[cfg(feature = "ace16-write-preview")]
#[test]
fn ace16_previews_share_nonunique_ordering_without_becoming_native_edit_plans() {
    for code in [5, 6] {
        for descending in [false, true] {
            for topology in 0..3 {
                let mut bytes = database(descending, true, false, topology);
                bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
                bytes[20] = code;
                let preview = capture(&bytes)
                    .preview_ace16_int32_updates(
                        "Items",
                        "Score",
                        &[Int32Update::new(1, 20, 30)],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(preview.changed_rows(), 1);
                assert_eq!(preview.before_bytes(), bytes);
                verify(
                    &bytes,
                    preview.candidate_bytes(),
                    descending,
                    false,
                    &[Int32Update::new(1, 20, 30)],
                );
            }
        }
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {cases:128,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4455504b455953),..Default::default()})]
    #[test]
    fn nonunique_batches_match_an_independent_nullable_row_reference_model(a in any::<i32>(), b in any::<i32>(), descending in any::<bool>(), nullable in any::<bool>(), deep in any::<bool>()) {
        let bytes=database(descending,true,nullable,if deep {2}else{1});
        let updates=[Int32Update::new(1,20,a),Int32Update::new(2,30,b)];
        let plan=capture(&bytes).plan_int32_updates("Items","Score",&updates,&mut budget(),&||false).unwrap();
        prop_assert_eq!(plan.changed_rows(),usize::from(a!=20)+usize::from(b!=30));
        verify(&bytes,plan.after_bytes(),descending,nullable,&updates);
    }
}
