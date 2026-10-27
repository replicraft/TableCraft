use super::*;
use model::Family;
use std::cell::Cell;
use tablecraft_ace::{ConstraintViolation, InspectError, WriteBlocker};
use tablecraft_schema::FieldConstraintViolation;

#[test]
fn closed_profiles_constraints_and_original_request_failures_remain_atomic() {
    let mut french = model::root_case(Family::Legacy, false).bytes;
    french[10 * 4096 + 111..10 * 4096 + 115].copy_from_slice(&[12, 4, 0, 0]);
    checked(&french);
    let mut ignored = model::root_case(Family::Modern, false).bytes;
    ignored[10 * 4096 + 189] = 0x82;
    let rows = read_table(
        &mut Cursor::new(&ignored),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows.rows().len(), 900);
    let physical = check_table(
        &mut Cursor::new(&ignored),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        physical
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(&ignored),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), 900);
    assert!(constraints.relationships().is_empty());
    // Physical membership can be checked, while an omitting index must never
    // masquerade as the public complete-table All read, even for these rows.
    for index in ["ScoreKey", "ScoreAlias"] {
        assert_eq!(
            read_table_index(
                &mut Cursor::new(&ignored),
                CatalogScope::new(1),
                "Items",
                index,
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported),
        );
    }
    let mut unknown = model::root_case(Family::Modern, false).bytes;
    unknown[10 * 4096 + 111..10 * 4096 + 115].copy_from_slice(&[31, 4, 0, 1]);
    let mut memo = encoded_model::null_memo().bytes;
    memo[10 * 4096 + 115] = 2;
    for bytes in [&french, &ignored, &unknown, &memo] {
        let old = encoded_model::capture(bytes)
            .preview_ace16_slot_updates("Items", "Score", &[], &mut budget(), &|| false)
            .err();
        assert!(old.is_some());
        assert_eq!(preview(bytes, &[]).err(), old);
    }
    let mut tuple = encoded_model::composite();
    tuple.bytes[10 * 4096 + 115] = 2;
    checked(&tuple.bytes);
    assert_eq!(
        encoded_model::capture(&tuple.bytes)
            .preview_ace16_slot_updates("Items", "Score", &[], &mut budget(), &|| false)
            .unwrap()
            .candidate_bytes(),
        tuple.bytes,
    );
    for requests in [
        vec![],
        vec![SlotScalarUpdate::new(
            2,
            Value::Text("a".into()),
            Value::Text("a".into()),
        )],
    ] {
        assert_eq!(
            preview(&tuple.bytes, &requests).err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
    }
    for (name, property, replacement, violation) in [
        (
            "Required",
            vec![255],
            Value::Null,
            FieldConstraintViolation::Required,
        ),
        (
            "AllowZeroLength",
            vec![0],
            Value::Text("".into()),
            FieldConstraintViolation::EmptyText,
        ),
    ] {
        let bytes = model::property_source(name, 1, &property);
        checked(&bytes);
        assert_eq!(
            preview(
                &bytes,
                &[SlotScalarUpdate::new(
                    0,
                    Value::Text("A".into()),
                    replacement
                )]
            )
            .err(),
            Some(EditError::Inspect(InspectError::Constraint(
                ConstraintViolation::Field(violation)
            ))),
        );
    }
    let rule = "StrComp([Score], 'A', 0) = 0"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let bytes = model::property_source("ValidationRule", 10, &rule);
    checked(&bytes);
    let requests = [SlotScalarUpdate::new(
        0,
        Value::Text("A".into()),
        Value::Text("a".into()),
    )];
    let old = encoded_model::capture(&bytes)
        .preview_ace16_slot_updates("Items", "Score", &requests, &mut budget(), &|| false)
        .err();
    assert!(old.is_some());
    assert_eq!(preview(&bytes, &requests).err(), old);

    let case = model::root_case(Family::Modern, false);
    for requests in [
        vec![SlotScalarUpdate::new(
            0,
            Value::Text("AAA".into()),
            case.new.clone(),
        )],
        vec![SlotScalarUpdate::new(
            900,
            case.old.clone(),
            case.new.clone(),
        )],
        vec![
            SlotScalarUpdate::new(0, case.old.clone(), case.new.clone()),
            SlotScalarUpdate::new(0, case.old.clone(), case.new.clone()),
        ],
    ] {
        assert_eq!(
            preview(&case.bytes, &requests).err(),
            Some(EditError::Conflict)
        );
    }
    assert_eq!(
        preview(
            &case.bytes,
            &[SlotScalarUpdate::new(
                0,
                case.old.clone(),
                Value::Binary(vec![1; 6])
            )]
        )
        .err(),
        Some(EditError::Unsupported(WriteBlocker::Column))
    );
    let small = model::small_source(Family::Modern, false);
    for (row, old, new) in [
        (0, Value::Null, Value::Text("x".into())),
        (2, Value::Text("a".into()), Value::Text("long".into())),
        (2, Value::Text("a".into()), Value::Text("\u{feff}".into())),
        (4, Value::Text("é".into()), Value::Text("e\u{301}".into())),
    ] {
        assert_eq!(
            preview(&small.bytes, &[SlotScalarUpdate::new(row, old, new)]).err(),
            Some(EditError::Unsupported(WriteBlocker::RowLayout))
        );
    }
    let mut malformed = case.bytes.clone();
    malformed[13 * 4096] = 99;
    let old = encoded_model::capture(&malformed)
        .preview_ace16_slot_updates("Items", "Score", &[], &mut budget(), &|| false)
        .err();
    assert!(old.is_some());
    assert_eq!(preview(&malformed, &[]).err(), old);
    let mut wrong_flags = case.bytes.clone();
    wrong_flags[10 * 4096 + 115] = 0;
    assert_eq!(
        preview(&wrong_flags, &[]).err(),
        Some(EditError::Unsupported(WriteBlocker::Column))
    );
    accepted(&case);
}

#[test]
fn quotas_late_cancellation_future_cells_and_retry_preserve_the_source() {
    let case = model::root_case(Family::Modern, false);
    let original = case.bytes.clone();
    let requests = [SlotScalarUpdate::new(0, case.old.clone(), case.new.clone())];
    for mut allowance in [
        ResourceBudget::new(0, 1_000_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 0),
    ] {
        assert!(matches!(
            encoded_model::capture(&case.bytes).preview_ace16_split_slot_updates(
                "Items",
                "Score",
                &requests,
                &mut allowance,
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_))),
        ));
    }
    let calls = Cell::new(0);
    let successful = encoded_model::capture(&case.bytes)
        .preview_ace16_split_slot_updates("Items", "Score", &requests, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(successful.candidate_bytes(), case.expected);
    let observed = calls.get();
    assert!(observed > 10);
    for checkpoint in [0, observed / 2, observed - 1] {
        let calls = Cell::new(0);
        let result = encoded_model::capture(&case.bytes).preview_ace16_split_slot_updates(
            "Items",
            "Score",
            &requests,
            &mut budget(),
            &|| {
                let current = calls.get();
                calls.set(current + 1);
                current == checkpoint
            },
        );
        assert_eq!(
            result.err(),
            Some(EditError::Inspect(InspectError::Cancelled))
        );
    }
    let mut missing = case.bytes.clone();
    missing[4096..8192].copy_from_slice(&support::data_page(1, &[(0, vec![1, 0, 0, 0, 0])]));
    checked(&missing);
    assert_eq!(
        preview(&missing, &requests).err(),
        Some(EditError::Inspect(InspectError::Unsupported))
    );
    let noop = preview(
        &missing,
        &[SlotScalarUpdate::new(0, case.old.clone(), case.old.clone())],
    )
    .unwrap();
    assert_eq!(noop.candidate_bytes(), missing);
    for page in [24, 25, 26] {
        let mut nonfree = case.bytes.clone();
        nonfree[4096 + 4027 + 5 + page / 8] &= !(1 << (page % 8));
        checked(&nonfree);
        assert_eq!(
            preview(&nonfree, &requests).err(),
            Some(EditError::Inspect(InspectError::Malformed))
        );
    }
    let mut second = model::two_owners(Family::Binary);
    checked(&second.bytes);
    second.bytes[4096 + 4027 + 5 + 29 / 8] &= !(1 << (29 % 8));
    assert_eq!(
        preview(
            &second.bytes,
            &[SlotScalarUpdate::new(0, second.old, second.new)]
        )
        .err(),
        Some(EditError::Inspect(InspectError::Malformed))
    );
    assert_eq!(case.bytes, original);
    accepted(&case);
}
