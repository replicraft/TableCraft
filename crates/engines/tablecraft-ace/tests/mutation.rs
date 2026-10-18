#[path = "mutation/fixture.rs"]
mod fixture;
mod support;

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
