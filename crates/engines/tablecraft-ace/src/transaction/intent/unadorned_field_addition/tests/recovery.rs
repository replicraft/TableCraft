//! Independently framed all-scalar originals reconstruct exact raw V3 effects.
use super::{Intent, SHAPES, Shape, budget, literal};
use crate::{
    CreateColumn, NativeImage, RecoverableUnadornedFieldAdditionPlan,
    UnadornedFieldAdditionDefinition, UnadornedFieldAdditionIntentError as Error,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::{
        declared_fields, fields, typed_fields::Kind, unadorned_fields as raw,
    },
    support::io::journal::MemoryStorage,
};

struct Case {
    before: Vec<u8>,
    definition: u32,
    table: &'static str,
    source: u8,
    order: [u8; 4],
}
fn cases() -> [Case; 2] {
    let plain = declared_fields::source();
    let generated = fields::source(fields::Case::FirstNumber);
    let mut cases = [
        Case {
            before: plain.bytes,
            definition: plain.definition,
            table: "Items",
            source: 1,
            order: [9, 4, 0, 1],
        },
        Case {
            before: generated.bytes,
            definition: generated.definition,
            table: "Table1",
            source: 2,
            order: [9, 4, 0, 1],
        },
    ];
    for case in &mut cases {
        set_root(case, [9, 4, 0, 1]);
    }
    cases
}
fn set_root(case: &mut Case, order: [u8; 4]) {
    for (index, mask) in [1, 27, 152, 253].into_iter().enumerate() {
        case.before[110 + index] = order[index] ^ mask;
    }
    case.order = order;
}
fn image(bytes: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}
fn kind(shape: Shape) -> Kind {
    match shape.tag {
        1 => Kind::Boolean,
        2 => Kind::Byte,
        3 => Kind::Int16,
        4 => Kind::Int32,
        19 => Kind::Int64,
        6 => Kind::Single,
        7 => Kind::Double,
        5 => Kind::Currency,
        8 => Kind::DateTime,
        15 => Kind::Guid,
        10 => Kind::ShortText {
            units: shape.width / 2,
            compression: false,
        },
        16 => Kind::Decimal {
            precision: shape.precision,
            scale: shape.scale,
        },
        _ => panic!("independent literal shape"),
    }
}
fn column(shape: Shape) -> CreateColumn<'static> {
    match shape.tag {
        1 => CreateColumn::new("AddedValue", ScalarType::Boolean),
        2 => CreateColumn::new("AddedValue", ScalarType::Byte),
        3 => CreateColumn::new("AddedValue", ScalarType::Int16),
        4 => CreateColumn::new("AddedValue", ScalarType::Int32),
        19 => CreateColumn::new("AddedValue", ScalarType::Int64),
        6 => CreateColumn::new("AddedValue", ScalarType::Single),
        7 => CreateColumn::new("AddedValue", ScalarType::Double),
        5 => CreateColumn::new("AddedValue", ScalarType::Currency),
        8 => CreateColumn::new("AddedValue", ScalarType::DateTime),
        15 => CreateColumn::new("AddedValue", ScalarType::Guid),
        10 => CreateColumn::short_text("AddedValue", usize::from(shape.width / 2)).unwrap(),
        16 => CreateColumn::decimal(
            "AddedValue",
            DecimalConstraints::new(shape.precision, shape.scale).unwrap(),
        ),
        _ => panic!("independent literal shape"),
    }
}
fn plan(case: &Case, shape: Shape) -> RecoverableUnadornedFieldAdditionPlan {
    let definition =
        UnadornedFieldAdditionDefinition::capture(column(shape), &mut budget(), &|| false).unwrap();
    image(&case.before, 30700)
        .plan_recoverable_retained_unadorned_field_addition(
            case.table,
            definition,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}
fn original(case: &Case, shape: Shape) -> Intent {
    Intent::decode(
        &literal(
            &case.before,
            case.table,
            case.definition,
            shape,
            case.source,
        ),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}
fn after(case: &Case, shape: Shape) -> Vec<u8> {
    raw::after(
        &case.before,
        case.definition,
        "AddedValue",
        kind(shape),
        case.order,
    )
}
fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn all_scalar_originals_recover_pending_mixed_committed_and_confirm_settled_without_sql() {
    for case in cases() {
        for shape in SHAPES {
            let after = after(&case, shape);
            let captured = plan(&case, shape);
            assert_eq!(
                captured.intent().encode(&mut budget(), &|| false).unwrap(),
                literal(
                    &case.before,
                    case.table,
                    case.definition,
                    shape,
                    case.source
                )
            );
            same(captured.after_bytes(), &after);
            for state in 0..3 {
                let mut target = MemoryStorage::new(case.before.to_vec());
                let mut journal_file = MemoryStorage::new(Vec::new());
                let prepared = plan(&case, shape)
                    .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
                    .unwrap();
                if state == 2 {
                    prepared.commit(&mut budget(), &|| false).unwrap();
                } else {
                    drop(prepared);
                    if state == 1 {
                        let base = case.definition as usize * 4096;
                        target.bytes.get_mut()[base..base + 100]
                            .copy_from_slice(&after[base..base + 100]);
                    }
                }
                let checked =
                    journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
                let qualified = original(&case, shape)
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(30701),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(
                    qualified.state(),
                    if state == 2 {
                        ResizableState::Committed
                    } else {
                        ResizableState::Pending
                    }
                );
                assert_eq!(
                    qualified
                        .recover_retained(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    if state == 2 {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    }
                );
                same(
                    target.bytes.get_ref(),
                    if state == 2 { &after } else { &case.before },
                );
                let writes = target.writes;
                let checked =
                    journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
                original(&case, shape)
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(30702),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(target.writes, writes);
            }
        }
    }
}

#[test]
fn source_class_identity_shape_and_whole_after_forgery_refuse_before_effects() {
    for case in cases() {
        let shape = SHAPES[11];
        let after = after(&case, shape);
        for offset in [24, 56, 60, 64, 66, 68, 76, 80, 81] {
            let mut wrong = literal(
                &case.before,
                case.table,
                case.definition,
                shape,
                case.source,
            );
            match offset {
                60 => {
                    let other_definition = if case.definition == 1 { 2_u32 } else { 1_u32 };
                    wrong[60..64].copy_from_slice(&other_definition.to_le_bytes());
                }
                76 => wrong[offset] = if case.source == 1 { 2 } else { 1 },
                80 => wrong[offset] = 27,
                _ => wrong[offset] ^= 1,
            }
            let intent = Intent::decode(&wrong, &mut budget(), &|| false)
                .unwrap_or_else(|error| panic!("canonical substituted offset {offset}: {error:?}"));
            let mut target = MemoryStorage::new(case.before.to_vec());
            let mut journal_file = MemoryStorage::new(Vec::new());
            drop(
                journal::prepare_resizable_v3_expected(
                    &mut target,
                    &mut journal_file,
                    &case.before,
                    &after,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            let refusal = intent.qualify_retained_recovery(
                checked,
                CatalogScope::new(30710),
                &mut budget(),
                &|| false,
            );
            if offset == 76 {
                assert!(matches!(refusal, Err(Error::IdentityMismatch)));
            } else {
                assert!(refusal.is_err(), "offset {offset}");
            }
            assert_eq!(target.writes, 0);
            same(target.bytes.get_ref(), &case.before);
        }
        for offset in [
            3 * 4096 + 3000,
            case.definition as usize * 4096 + 3000,
            (case.definition as usize + 1) * 4096 + 4000,
        ] {
            let mut forged = after.to_vec();
            forged[offset] ^= 1;
            let mut target = MemoryStorage::new(case.before.to_vec());
            let mut journal_file = MemoryStorage::new(Vec::new());
            drop(
                journal::prepare_resizable_v3_expected(
                    &mut target,
                    &mut journal_file,
                    &case.before,
                    &forged,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            assert!(
                original(&case, shape)
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(30711),
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
            assert_eq!(target.writes, 0);
        }
    }
}

#[test]
fn fresh_requalification_shares_after_and_refuses_stale_target_legacy_and_unpaid_proof() {
    let [mut case, _] = cases();
    set_root(&mut case, [9, 4, 0, 0]);
    let shape = SHAPES[10];
    let after = after(&case, shape);
    let captured = plan(&case, shape);
    let shared = captured.after_shared_bytes();
    assert_eq!(shared.as_ptr(), captured.after_bytes().as_ptr());
    let rebuilt = captured
        .requalify_with_budgets(
            image(&case.before, 30720),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    same(rebuilt.after_bytes(), &after);
    drop(rebuilt);
    same(&shared, &after);
    assert!(
        plan(&case, shape)
            .requalify_with_budgets(
                image(&case.before, 30721),
                &mut budget(),
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
    let mut stale = case.before.to_vec();
    stale[3 * 4096 + 3000] ^= 1;
    let mut target = MemoryStorage::new(stale.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(
        plan(&case, shape)
            .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, journal_file.writes), (0, 0));
    same(target.bytes.get_ref(), &stale);
    for version in [1, 2] {
        let mut target = MemoryStorage::new(case.before.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        if version == 1 {
            drop(
                journal::prepare_expected(
                    &mut target,
                    &mut journal_file,
                    &case.before,
                    &after,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
        } else {
            drop(
                journal::prepare_resizable_expected(
                    &mut target,
                    &mut journal_file,
                    &case.before,
                    &after,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
        }
        let writes = journal_file.writes;
        assert!(journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).is_err());
        assert_eq!(target.writes, 0);
        assert_eq!(journal_file.writes, writes);
    }
}
