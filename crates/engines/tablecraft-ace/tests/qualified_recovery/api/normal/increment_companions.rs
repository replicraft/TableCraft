//! Original presence, exact typed companions, LAST and shared physical aliases.
#[path = "increment_companions/recovery.rs"]
mod recovery;
#[path = "increment_companions/resources.rs"]
mod resources;

use super::{Storage, budget, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexConstraintViolation, InsertFieldValue, InspectError, NativeImage,
    NativeOmittedInsertIntent, OmittedRowInsert, RecoverableEditError, WriteBlocker,
    inspect_table_index_counter, read_table, read_table_index,
};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_increment_companions as fixtures;

fn cases() -> Vec<fixtures::Case> {
    fixtures::load_all(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn requests(case: &fixtures::Case, empty: bool) -> Vec<OmittedRowInsert> {
    if empty {
        return Vec::new();
    }
    vec![OmittedRowInsert::new(
        case.presence()
            .into_iter()
            .map(|value| value.map_or(InsertFieldValue::Omitted, InsertFieldValue::Supplied))
            .collect(),
    )]
}

fn wire(case: &fixtures::Case, empty: bool) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    let rows = if empty {
        Vec::new()
    } else {
        vec![case.presence()]
    };
    let raw = tablecraft_testkit::fixtures::access::tcna2::original_presence_wire(
        case.table,
        case.before.len(),
        digest.finalize().into(),
        rows,
    );
    let expected = if empty {
        if case.family == fixtures::Family::Middle {
            76
        } else {
            73
        }
    } else {
        match (case.family, case.state) {
            (fixtures::Family::Text, "companion-null-high") => 109,
            (fixtures::Family::Text, _) => 112,
            (fixtures::Family::Binary, "companion-null-high") => 109,
            (fixtures::Family::Binary, _) => 113,
            (fixtures::Family::Fixed, "companion-null-high") => 106,
            (fixtures::Family::Fixed, _) => 114,
            (fixtures::Family::Middle, _) => 116,
        }
    };
    assert_eq!(raw.len(), expected);
    raw
}

#[test]
#[ignore = "requires independently qualified original20/AFTER20 sources before matching generation"]
fn companion_presence_commits_twenty_exact_images_and_preserves_twenty_empty_sources() {
    let cases = cases();
    let mut count = 0;
    for case in &cases {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                case.table,
                requests(case, false),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before.as_slice());
        assert_eq!(plan.after_bytes(), case.after.as_slice());
        assert_eq!(plan.inserted_rows(), 1);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(case, false)
        );
        let decoded =
            NativeOmittedInsertIntent::decode(&wire(case, false), &mut budget(), &|| false)
                .unwrap();
        assert_eq!(
            decoded.encode(&mut budget(), &|| false).unwrap(),
            wire(case, false)
        );
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(target.bytes.get_ref().as_slice(), case.after.as_slice());
        let saved = image(target.bytes.get_ref());
        assert_eq!(
            read_table(
                &mut saved.reader(),
                scope(),
                case.table,
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            case.rows(true)
        );
        for alias in case.indexes() {
            assert_eq!(
                read_table_index(
                    &mut saved.reader(),
                    scope(),
                    case.table,
                    alias,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                case.rows(true)
            );
            assert_eq!(
                inspect_table_index_counter(
                    &mut saved.reader(),
                    scope(),
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
        let empty = image(&case.before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                case.table,
                requests(case, true),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(empty.after_bytes(), case.before.as_slice());
        assert_eq!(empty.inserted_rows(), 0);
        assert_eq!(
            empty.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(case, true)
        );
        count += 1;
    }
    assert_eq!(count, 20);
}

#[test]
#[ignore = "requires healthy source-bound companion declarations; every refusal is before publication"]
fn supplied_generated_values_missing_companions_carriers_and_empty_text_refuse() {
    let cases = cases();
    let mut refused = 0;
    for case in &cases {
        for value in [Value::Null, Value::Int32(case.generated)] {
            let mut values = case.presence();
            values[case.generated_position] = Some(value);
            let request = OmittedRowInsert::new(
                values
                    .into_iter()
                    .map(|value| {
                        value.map_or(InsertFieldValue::Omitted, InsertFieldValue::Supplied)
                    })
                    .collect(),
            );
            assert!(matches!(
                image(&case.before).plan_recoverable_ordinary_omitted_row_inserts(
                    case.table,
                    vec![request],
                    &mut budget(),
                    &|| false
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::Column
                )))
            ));
            refused += 1;
        }
        let ordinary = usize::from(case.generated_position == 0);
        let wrong_carrier = if case.family == fixtures::Family::Middle {
            Value::Text("wrong".into())
        } else {
            Value::Int32(9)
        };
        for value in [None, Some(wrong_carrier)] {
            let mut values = case.presence();
            values[ordinary] = value;
            let request = OmittedRowInsert::new(
                values
                    .into_iter()
                    .map(|value| {
                        value.map_or(InsertFieldValue::Omitted, InsertFieldValue::Supplied)
                    })
                    .collect(),
            );
            assert!(matches!(
                image(&case.before).plan_recoverable_ordinary_omitted_row_inserts(
                    case.table,
                    vec![request],
                    &mut budget(),
                    &|| false
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::Column
                )))
            ));
            refused += 1;
        }
        if case.family == fixtures::Family::Text {
            let mut values = case.presence();
            values[1] = Some(Value::Text(String::new()));
            let request = OmittedRowInsert::new(
                values
                    .into_iter()
                    .map(|value| {
                        value.map_or(InsertFieldValue::Omitted, InsertFieldValue::Supplied)
                    })
                    .collect(),
            );
            assert!(matches!(
                image(&case.before).plan_recoverable_ordinary_omitted_row_inserts(
                    case.table,
                    vec![request],
                    &mut budget(),
                    &|| false
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::ConstraintCoverage
                )))
            ));
            refused += 1;
        }
        if case.family == fixtures::Family::Fixed {
            let mut values = case.presence();
            values[1] = Some(Value::Null);
            let request = OmittedRowInsert::new(
                values
                    .into_iter()
                    .map(|value| {
                        value.map_or(InsertFieldValue::Omitted, InsertFieldValue::Supplied)
                    })
                    .collect(),
            );
            assert!(matches!(
                image(&case.before).plan_recoverable_ordinary_omitted_row_inserts(
                    case.table,
                    vec![request],
                    &mut budget(),
                    &|| false
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::Column
                )))
            ));
            refused += 1;
        }
    }
    assert_eq!(refused, 92);
}

#[test]
#[ignore = "requires healthy original companions; exact LAST is never repaired from live keys"]
fn source_relative_collision_and_signed_exhaustion_are_precise() {
    let cases = cases();
    let mut collisions = 0;
    for case in &cases {
        let at = case.definition as usize * 4096 + 20;
        let mut exhausted = case.before.clone();
        exhausted[at..at + 4].copy_from_slice(&i32::MAX.to_le_bytes());
        assert_eq!(
            read_table(
                &mut image(&exhausted).reader(),
                scope(),
                case.table,
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            case.rows(false)
        );
        assert!(matches!(
            image(&exhausted).plan_recoverable_ordinary_omitted_row_inserts(
                case.table,
                requests(case, false),
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::AutoNumberExhausted))
        ));
        if case.old_live != 0 {
            let mut collision = case.before.clone();
            collision[at..at + 4].copy_from_slice(&0_i32.to_le_bytes());
            assert_eq!(
                read_table(
                    &mut image(&collision).reader(),
                    scope(),
                    case.table,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                case.rows(false)
            );
            assert!(matches!(
                image(&collision).plan_recoverable_ordinary_omitted_row_inserts(
                    case.table,
                    requests(case, false),
                    &mut budget(),
                    &|| false
                ),
                Err(RecoverableEditError::Edit(EditError::Inspect(
                    InspectError::IndexConstraint(IndexConstraintViolation::DuplicateKey)
                )))
            ));
            collisions += 1;
        }
    }
    assert_eq!(collisions, 14);
}
