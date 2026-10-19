#[path = "mutation/fixture.rs"]
mod fixture;
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[path = "mutation/non_unique.rs"]
mod non_unique;
mod support;
#[path = "mutation/tree_fixture.rs"]
mod tree_fixture;

use proptest::prelude::*;
use std::io::Cursor;
use tablecraft_ace::{EditError, Int32Update, NativeImage, WriteBlocker, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}
fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn unique_leaf_key_updates_resort_references_and_preserve_index_metadata_and_opaque_bytes() {
    for descending in [false, true] {
        for alias in [false, true] {
            for nullable in [false, true] {
                let bytes = index_fixture::database(descending, alias, nullable);
                let plan = capture(&bytes)
                    .plan_int32_updates(
                        "Items",
                        "Score",
                        &[Int32Update::new(1, 20, 35)],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(plan.changed_rows(), 1);
                let rows = tablecraft_ace::read_table_index(
                    &mut Cursor::new(plan.after_bytes()),
                    CatalogScope::new(1),
                    "Items",
                    "ScoreKey",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let mut expected = if nullable {
                    vec![Value::Null, Value::Int32(30), Value::Int32(35)]
                } else {
                    vec![Value::Int32(10), Value::Int32(30), Value::Int32(35)]
                };
                if descending {
                    expected.reverse();
                }
                assert_eq!(
                    rows.rows()
                        .iter()
                        .map(|row| row[1].clone())
                        .collect::<Vec<_>>(),
                    expected
                );
                if alias {
                    assert!(
                        tablecraft_ace::read_table_index(
                            &mut Cursor::new(plan.after_bytes()),
                            CatalogScope::new(1),
                            "Items",
                            "ScoreAlias",
                            &mut budget(),
                            &|| false
                        )
                        .is_ok()
                    );
                }
                let report = tablecraft_ace::check_table(
                    &mut Cursor::new(plan.after_bytes()),
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
                        .all(|index| index.status() == tablecraft_ace::IndexCheckStatus::Checked)
                );
                let start = 12 * 4096
                    + u16::from_le_bytes(bytes[12 * 4096 + 16..12 * 4096 + 18].try_into().unwrap())
                        as usize;
                let end = 14 * 4096
                    - u16::from_le_bytes(bytes[13 * 4096 + 2..13 * 4096 + 4].try_into().unwrap())
                        as usize;
                for (offset, (before, after)) in bytes.iter().zip(plan.after_bytes()).enumerate() {
                    if !(start + 6..start + 10).contains(&offset)
                        && !(13 * 4096 + 480..end).contains(&offset)
                    {
                        assert_eq!(before, after, "opaque edit {offset}");
                    }
                }
            }
        }
    }
}

#[test]
fn unique_duplicates_prefix_changes_and_unknown_headers_publish_no_plan() {
    let bytes = index_fixture::database(false, false, false);
    assert_eq!(
        capture(&bytes)
            .plan_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(1, 20, 30)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Inspect(
            tablecraft_ace::InspectError::IndexConstraint(
                tablecraft_ace::IndexConstraintViolation::DuplicateKey
            )
        ))
    );
    assert_eq!(
        capture(&bytes)
            .plan_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(1, 20, i32::MAX)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
    );
    for offset in [13 * 4096 + 8, 13 * 4096 + 26] {
        let mut unknown = bytes.clone();
        unknown[offset] = 1;
        assert_eq!(
            capture(&unknown)
                .plan_int32_updates(
                    "Items",
                    "Score",
                    &[Int32Update::new(1, 20, 35)],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
    }
    let noop = capture(&bytes)
        .plan_int32_updates(
            "Items",
            "Score",
            &[Int32Update::new(1, 20, 20)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(noop.before_bytes(), noop.after_bytes());
    assert_eq!(noop.changed_rows(), 0);
}

#[test]
fn multi_page_unique_trees_move_keys_across_leaves_and_update_separator_row_references() {
    let bytes = index_fixture::node_database();
    assert!(
        tablecraft_ace::check_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .is_ok()
    );
    for updates in [
        vec![Int32Update::new(1, 20, 35)],
        vec![Int32Update::new(2, 30, -1)],
        vec![Int32Update::new(0, 10, i32::MAX)],
        vec![Int32Update::new(0, 10, 30), Int32Update::new(2, 30, 10)],
    ] {
        let plan = capture(&bytes)
            .plan_int32_updates("Items", "Score", &updates, &mut budget(), &|| false)
            .unwrap();
        let mut model = vec![(10, 1), (20, 2), (30, 3)];
        for update in &updates {
            model[update.row()].0 = update.value();
        }
        model.sort_unstable();
        let indexed = tablecraft_ace::read_table_index(
            &mut Cursor::new(plan.after_bytes()),
            CatalogScope::new(1),
            "Items",
            "ScoreKey",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            indexed
                .rows()
                .iter()
                .map(|row| match (&row[1], &row[0]) {
                    (Value::Int32(key), Value::Int32(id)) => (*key, *id),
                    _ => panic!("carriers"),
                })
                .collect::<Vec<_>>(),
            model
        );
        // Every page header, original bitmap, all unused bytes and allocation
        // metadata remain byte-identical even when a root separator changes.
        let allowed = [
            12 * 4096..13 * 4096,
            13 * 4096 + 480..13 * 4096 + 493,
            15 * 4096 + 480..15 * 4096 + 498,
            16 * 4096 + 480..16 * 4096 + 489,
        ];
        for (offset, (before, after)) in bytes.iter().zip(plan.after_bytes()).enumerate() {
            if !allowed.iter().any(|span| span.contains(&offset)) {
                assert_eq!(before, after, "unexpected byte {offset}");
            }
        }
    }
    let duplicate = capture(&bytes).plan_int32_updates(
        "Items",
        "Score",
        &[Int32Update::new(0, 10, 30)],
        &mut budget(),
        &|| false,
    );
    assert_eq!(
        duplicate.err(),
        Some(EditError::Inspect(
            tablecraft_ace::InspectError::IndexConstraint(
                tablecraft_ace::IndexConstraintViolation::DuplicateKey
            )
        ))
    );
    for offset in [13 * 4096 + 8, 13 * 4096 + 26, 15 * 4096 + 26] {
        let mut unknown = bytes.clone();
        unknown[offset] ^= 1;
        assert_eq!(
            capture(&unknown)
                .plan_int32_updates(
                    "Items",
                    "Score",
                    &[Int32Update::new(1, 20, 35)],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
    }
    let noop = capture(&bytes)
        .plan_int32_updates(
            "Items",
            "Score",
            &[Int32Update::new(1, 20, 20)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(noop.before_bytes(), noop.after_bytes());
}

#[test]
fn indexed_tree_plans_publish_nothing_on_late_cancellation_or_budget_exhaustion() {
    use std::cell::Cell;
    use tablecraft_core::limits::Resource;
    let bytes = tree_fixture::database(true, true, true);
    let calls = Cell::new(0_usize);
    let mut measured = budget();
    capture(&bytes)
        .plan_int32_updates(
            "Items",
            "Score",
            &[Int32Update::new(1, 20, i32::MIN)],
            &mut measured,
            &|| {
                calls.set(calls.get() + 1);
                false
            },
        )
        .unwrap();
    for stop in [1, calls.get() / 4, calls.get() / 2, calls.get() - 1] {
        let current = Cell::new(0);
        assert_eq!(
            capture(&bytes)
                .plan_int32_updates(
                    "Items",
                    "Score",
                    &[Int32Update::new(1, 20, i32::MIN)],
                    &mut budget(),
                    &|| {
                        current.set(current.get() + 1);
                        current.get() >= stop
                    }
                )
                .err(),
            Some(EditError::Inspect(tablecraft_ace::InspectError::Cancelled))
        );
    }
    let used = budget().remaining(Resource::Bytes) - measured.remaining(Resource::Bytes);
    let mut limited = ResourceBudget::new(used - 1, 100_000, 10_000_000);
    assert!(matches!(
        capture(&bytes).plan_int32_updates(
            "Items",
            "Score",
            &[Int32Update::new(1, 20, i32::MIN)],
            &mut limited,
            &|| false
        ),
        Err(EditError::Inspect(tablecraft_ace::InspectError::Limit(_)))
    ));
}

#[test]
fn valid_index_rewrites_still_fail_the_whole_plan_when_a_field_rule_rejects_the_value() {
    let bytes = index_fixture::with_rule(false, false, false, "ValidationRule", "<=30");
    assert_eq!(
        capture(&bytes)
            .plan_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(1, 20, 35)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Inspect(
            tablecraft_ace::InspectError::Constraint(
                tablecraft_ace::ConstraintViolation::FieldRule
            )
        ))
    );
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x494e444558575249),..Default::default()})]
    #[test]
    fn indexed_updates_follow_an_independent_row_key_model(
        replacement in prop_oneof![0_i32..256,any::<i32>()],descending in any::<bool>(),alias in any::<bool>(),nullable in any::<bool>()) {
        let bytes=index_fixture::database(descending,alias,nullable);
        let result=capture(&bytes).plan_int32_updates("Items","Score",&[Int32Update::new(1,20,replacement)],&mut budget(),&||false);
        if replacement==30 || (!nullable&&replacement==10) {
            prop_assert_eq!(result.err(),Some(EditError::Inspect(tablecraft_ace::InspectError::IndexConstraint(tablecraft_ace::IndexConstraintViolation::DuplicateKey))));
        } else if !nullable && !(0..256).contains(&replacement) {
            // The independent golden has a four-byte common prefix for small
            // positive keys; other values require an unqualified prefix change.
            prop_assert_eq!(result.err(),Some(EditError::Unsupported(WriteBlocker::IndexedColumn)));
        } else {
            let plan=result.unwrap();
            prop_assert_eq!(plan.changed_rows(),usize::from(replacement!=20));
            let indexed=tablecraft_ace::read_table_index(&mut Cursor::new(plan.after_bytes()),CatalogScope::new(1),"Items","ScoreKey",&mut budget(),&||false).unwrap();
            let mut expected=vec![(if nullable{None}else{Some(10)},1),(Some(replacement),2),(Some(30),3)];
            expected.sort_by_key(|pair|pair.0);if descending {expected.reverse();}
            let actual=indexed.rows().iter().map(|row|{
                let key=match row[1]{Value::Null=>None,Value::Int32(key)=>Some(key),_=>panic!("key carrier")};
                let Value::Int32(id)=row[0]else{panic!("id carrier")};(key,id)
            }).collect::<Vec<_>>();
            prop_assert_eq!(actual,expected);
        }
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x545245454b455953),..Default::default()})]
    #[test]
    fn existing_tree_topologies_match_a_sorted_row_model(
        replacement in any::<i32>(), descending in any::<bool>(), nullable in any::<bool>(), deep in any::<bool>()) {
        let bytes=tree_fixture::database(descending,nullable,deep);
        let result=capture(&bytes).plan_int32_updates("Items","Score",&[Int32Update::new(1,20,replacement)],&mut budget(),&||false);
        if replacement==30||(!nullable&&replacement==10) {
            prop_assert_eq!(result.err(),Some(EditError::Inspect(tablecraft_ace::InspectError::IndexConstraint(tablecraft_ace::IndexConstraintViolation::DuplicateKey))));
        } else {
            let plan=result.unwrap();
            let indexed=tablecraft_ace::read_table_index(&mut Cursor::new(plan.after_bytes()),CatalogScope::new(1),"Items","ScoreKey",&mut budget(),&||false).unwrap();
            let mut expected=vec![(if nullable{None}else{Some(10)},1),(Some(replacement),2),(Some(30),3)];
            expected.sort_by_key(|pair|pair.0);if descending{expected.reverse();}
            let actual=indexed.rows().iter().map(|row|{
                let key=match row[1]{Value::Null=>None,Value::Int32(key)=>Some(key),_=>panic!("key carrier")};
                let Value::Int32(id)=row[0] else {panic!("id carrier")};(key,id)
            }).collect::<Vec<_>>();
            prop_assert_eq!(actual,expected);
            for number in [13,15,16,17,18,19].into_iter().filter(|number|number*4096<bytes.len()) {
                prop_assert_eq!(&bytes[number*4096..number*4096+480],&plan.after_bytes()[number*4096..number*4096+480]);
                let end=4096-u16::from_le_bytes(bytes[number*4096+2..number*4096+4].try_into().unwrap()) as usize;
                prop_assert_eq!(&bytes[number*4096+end..(number+1)*4096],&plan.after_bytes()[number*4096+end..(number+1)*4096]);
            }
        }
    }
}

#[test]
fn fixed_updates_reopen_and_preserve_every_other_byte() {
    let bytes = fixture::database();
    let plan = capture(&bytes)
        .plan_int32_updates(
            "Items",
            "Score",
            &[
                Int32Update::new(0, 10, -7),
                Int32Update::new(2, 30, i32::MAX),
            ],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.changed_rows(), 2);
    assert_eq!(plan.before_bytes(), bytes);
    assert_eq!(plan.after_bytes().len(), bytes.len());
    let table = read_table(
        &mut Cursor::new(plan.after_bytes()),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        table.rows(),
        &[
            vec![Value::Int32(1), Value::Int32(-7)],
            vec![Value::Int32(2), Value::Int32(20)],
            vec![Value::Int32(3), Value::Int32(i32::MAX)]
        ]
    );
    let allowed = [
        12 * 4096 + 4096 - 11 + 6..12 * 4096 + 4096 - 11 + 10,
        12 * 4096 + 4096 - 33 + 6..12 * 4096 + 4096 - 33 + 10,
    ];
    for (offset, (before, after)) in bytes.iter().zip(plan.after_bytes()).enumerate() {
        if !allowed.iter().any(|range| range.contains(&offset)) {
            assert_eq!(before, after, "unexpected edit at {offset}");
        }
    }
    assert!(
        plan.after_bytes()[13 * 4096..]
            .iter()
            .all(|byte| *byte == 0xa7)
    );
}

#[test]
fn stale_repeated_absent_and_null_updates_fail_without_a_plan() {
    let bytes = fixture::database();
    for updates in [
        vec![Int32Update::new(0, 11, 2)],
        vec![Int32Update::new(3, 0, 1)],
        vec![Int32Update::new(0, 10, 1), Int32Update::new(0, 10, 2)],
    ] {
        assert_eq!(
            capture(&bytes)
                .plan_int32_updates("Items", "Score", &updates, &mut budget(), &|| false)
                .err(),
            Some(EditError::Conflict)
        );
    }
    let mut null = bytes.clone();
    null[13 * 4096 - 1] &= !2;
    assert_eq!(
        capture(&null)
            .plan_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(0, 10, 1)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Conflict)
    );
    let plan = capture(&bytes)
        .plan_int32_updates(
            "Items",
            "Score",
            &[Int32Update::new(0, 10, 10)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.changed_rows(), 0);
    assert_eq!(plan.after_bytes(), bytes);
}

#[test]
fn unqualified_formats_autonumber_and_cancellation_block_edits() {
    let bytes = fixture::database();
    let mut alias = bytes.clone();
    let offset = usize::from(u16::from_le_bytes(
        alias[4 * 4096 + 16..4 * 4096 + 18].try_into().unwrap(),
    )) & 0x1fff;
    // Distinct catalog ID with the same 24-bit definition page, unflagged.
    support::put32(&mut alias, 4 * 4096 + offset + 2, 0x0100000a);
    support::put32(&mut alias, 4 * 4096 + offset + 12, 0);
    assert_eq!(
        capture(&alias)
            .plan_int32_updates("Items", "Score", &[], &mut budget(), &|| false)
            .err(),
        Some(EditError::Unsupported(WriteBlocker::Table))
    );
    let mut ace = bytes.clone();
    ace[4..20].copy_from_slice(b"Standard ACE DB\0");
    ace[20] = 2;
    assert_eq!(
        capture(&ace)
            .plan_int32_updates("Items", "Score", &[], &mut budget(), &|| false)
            .err(),
        Some(EditError::Unsupported(WriteBlocker::Format))
    );
    let mut auto = bytes.clone();
    auto[10 * 4096 + 63 + 25 + 15] |= 4;
    assert_eq!(
        capture(&auto)
            .plan_int32_updates("Items", "Score", &[], &mut budget(), &|| false)
            .err(),
        Some(EditError::Unsupported(WriteBlocker::Column))
    );
    assert!(matches!(
        capture(&bytes).plan_int32_updates("Items", "Score", &[], &mut budget(), &|| true),
        Err(EditError::Inspect(tablecraft_ace::InspectError::Cancelled))
    ));
}

#[test]
fn post_edit_rules_and_unqualified_declarations_block_the_whole_plan() {
    let bytes = fixture::with_rule("ValidationRule", ">=0 And <=100");
    assert!(
        capture(&bytes)
            .plan_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(1, 20, 99)],
                &mut budget(),
                &|| false
            )
            .is_ok()
    );
    assert_eq!(
        capture(&bytes)
            .plan_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(1, 20, -1)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Inspect(
            tablecraft_ace::InspectError::Constraint(
                tablecraft_ace::ConstraintViolation::FieldRule
            )
        ))
    );
    let bytes = fixture::with_rule("ValidationRule", "Abs([Score]) > 0");
    assert_eq!(
        capture(&bytes)
            .plan_int32_updates("Items", "Score", &[], &mut budget(), &|| false)
            .err(),
        Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    );
    let bytes = fixture::with_rule("FutureRule", "0");
    assert_eq!(
        capture(&bytes)
            .plan_int32_updates("Items", "Score", &[], &mut budget(), &|| false)
            .err(),
        Some(EditError::Unsupported(WriteBlocker::Properties))
    );
}

#[test]
fn opaque_table_payload_hints_block_writes_without_decoding_the_stream() {
    use tablecraft_ace::PayloadPresence;
    let mut bytes = fixture::database();
    let image = capture(&bytes);
    let object = image
        .catalog()
        .catalog()
        .lookup("Items", None, None)
        .unwrap()
        .unwrap()
        .id();
    let payloads = image
        .inspect_object_payloads(object, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        (payloads.body(), payloads.module(), payloads.extra()),
        (
            PayloadPresence::Absent,
            PayloadPresence::Absent,
            PayloadPresence::Absent
        )
    );
    // Independent third catalog slot: mark LvExtra non-NULL. Its malformed body
    // is intentionally never followed; a presence hint is enough to block edits.
    let start = usize::from(u16::from_le_bytes(
        bytes[4 * 4096 + 18..4 * 4096 + 20].try_into().unwrap(),
    )) & 0x1fff;
    let end = usize::from(u16::from_le_bytes(
        bytes[4 * 4096 + 16..4 * 4096 + 18].try_into().unwrap(),
    )) & 0x1fff;
    bytes[4 * 4096 + start..4 * 4096 + start + 2].copy_from_slice(&8_u16.to_le_bytes());
    bytes[4 * 4096 + end - 1] |= 0x80;
    let image = capture(&bytes);
    assert_eq!(
        image
            .inspect_object_payloads(object, &mut budget(), &|| false)
            .unwrap()
            .extra(),
        PayloadPresence::Present
    );
    assert_eq!(
        image
            .plan_int32_updates("Items", "Score", &[], &mut budget(), &|| false)
            .err(),
        Some(EditError::Unsupported(WriteBlocker::ActiveContent))
    );
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4649584544454449),..Default::default()})]
    #[test]
    fn arbitrary_int32_replacements_match_an_independent_row_model(row in 0_usize..3, value in any::<i32>()) {
        let bytes = fixture::database();
        let expected = [10, 20, 30][row];
        let plan = capture(&bytes).plan_int32_updates("Items", "Score", &[Int32Update::new(row, expected, value)], &mut budget(), &|| false).unwrap();
        let table = read_table(&mut Cursor::new(plan.after_bytes()), CatalogScope::new(1), "Items", &mut budget(), &|| false).unwrap();
        for (position, record) in table.rows().iter().enumerate() {
            prop_assert_eq!(&record[0], &Value::Int32(position as i32 + 1));
            prop_assert_eq!(&record[1], &Value::Int32(if position == row { value } else { [10,20,30][position] }));
        }
        prop_assert_eq!(plan.changed_rows(), usize::from(value != expected));
        prop_assert!(bytes.iter().zip(plan.after_bytes()).filter(|(before, after)| before != after).count() <= 4);
    }
}

#[test]
fn staged_fixed_edits_recheck_range_and_list_rules_before_publishing_any_candidate() {
    for rule in ["Between 0 And 100", "Between 100 And 0", "In (10,20,30)"] {
        let bytes = fixture::with_rule("ValidationRule", rule);
        let plan = capture(&bytes)
            .plan_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(0, 10, 30)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(plan.before_bytes(), bytes);
        assert_eq!(
            capture(&bytes)
                .plan_int32_updates(
                    "Items",
                    "Score",
                    &[Int32Update::new(0, 10, 200)],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Inspect(
                tablecraft_ace::InspectError::Constraint(
                    tablecraft_ace::ConstraintViolation::FieldRule
                )
            ))
        );
    }
}

#[test]
fn separately_owned_unknown_or_active_catalog_objects_block_otherwise_qualified_edits() {
    for kind in [-32766_i16, -32761, 1234, 1] {
        let mut bytes = fixture::database();
        support::put32(&mut bytes, 2 * 4096 + 16, 4);
        let objects = [
            support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
            support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
            support::record(10, 0, 1, 0, b"\xff\xfeItems"),
            support::record(100, 10, kind, 0, b"\xff\xfeOwnedObject"),
        ]
        .into_iter()
        .map(|r| (0, r))
        .collect::<Vec<_>>();
        bytes[4 * 4096..5 * 4096].copy_from_slice(&support::data_page(2, &objects));
        assert_eq!(
            capture(&bytes)
                .plan_int32_updates(
                    "Items",
                    "Score",
                    &[Int32Update::new(0, 10, 11)],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::ActiveContent))
        );
    }
}
