//! Literal complete images reach fresh proof directly, without a producer plan.
use super::proof;
use crate::{EditError, NativeEditPlan, RowInsert};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_ordinary_wide_insert::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(217);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn plan(source: &sources::Source) -> NativeEditPlan {
    NativeEditPlan {
        before: source.before.clone().into(),
        after: source.after.clone(),
        changed_rows: 1,
    }
}

fn check(plan: &NativeEditPlan, values: &[Value]) -> Result<(), EditError> {
    proof::prove(
        plan,
        SCOPE,
        "Items",
        &[RowInsert::new(values.to_vec())],
        &mut budget(),
        &|| false,
    )
}

fn header(source: &mut sources::Source, value: u32) {
    source.before[20..24].copy_from_slice(&value.to_le_bytes());
    source.after[20..24].copy_from_slice(&value.to_le_bytes());
}

fn decimal(case: Case) -> bool {
    matches!(
        case,
        Case::DecimalExact | Case::DecimalMax | Case::DecimalNull
    )
}

fn new_record(case: Case) -> (usize, usize) {
    match case {
        Case::DecimalExact => (8 * 4096 + 3951, 43),
        Case::DecimalMax => (8 * 4096 + 3960, 34),
        Case::DecimalNull => (8 * 4096 + 3977, 17),
        Case::EofFixed => (9 * 4096 + 4033, 63),
        _ => (8 * 4096 + 3907, 63),
    }
}

fn short(raw: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(raw[at..at + 2].try_into().unwrap()))
}

#[test]
fn all_sixteen_literal_scalar_afters_pass_complete_proof_without_staging() {
    let mut views = 0;
    for case in sources::CASES {
        for code in [5_u32, 6] {
            let mut source = sources::source(case);
            header(&mut source, code);
            check(&plan(&source), &source.values).unwrap();
            views += 1;
        }
    }
    assert_eq!(views, 16);
}

#[test]
fn complete_scalar_proof_refuses_wrong_counts_requests_and_valid_changed_before_values() {
    let mut controls = 0;
    for case in sources::CASES {
        for code in [5_u32, 6] {
            let mut source = sources::source(case);
            header(&mut source, code);
            let mut literal = plan(&source);
            for count in [0, 2] {
                literal.changed_rows = count;
                assert!(matches!(
                    check(&literal, &source.values),
                    Err(EditError::Preservation)
                ));
                controls += 1;
            }
            literal.changed_rows = 1;
            let mut wrong = source.values.clone();
            if decimal(case) {
                wrong[3] = Value::Decimal(Decimal::new(0, 4).unwrap());
            } else {
                wrong[1] = Value::Byte(if matches!(case, Case::FixedNull) {
                    0
                } else {
                    254
                });
            }
            assert!(matches!(
                check(&literal, &wrong),
                Err(EditError::Preservation)
            ));
            controls += 1;

            // Change an original, still representable value. The observed AFTER
            // retains its literal old value and must not pass fresh replay.
            let offset = if decimal(case) {
                8 * 4096 + 4064
            } else {
                8 * 4096 + 4035
            };
            let mut altered_before = literal.before.to_vec();
            altered_before[offset] ^= 1;
            literal.before = altered_before.into();
            assert!(matches!(
                check(&literal, &source.values),
                Err(EditError::Preservation)
            ));
            controls += 1;
        }
    }
    assert_eq!(controls, 64);
}

#[test]
fn complete_scalar_proof_closes_native_field_mask_padding_maps_header_and_unused_eof_bytes() {
    let mut controls = 0;
    for case in sources::CASES {
        for code in [5_u32, 6] {
            let mut source = sources::source(case);
            header(&mut source, code);
            let (record, width) = new_record(case);
            let mut offsets = vec![
                record,
                record + width - 1,
                24,
                4 * 4096 + 15,
                8 * 4096 + 4095,
            ];
            if decimal(case) {
                // The selected native NUMERIC sign/limbs are active only in
                // the exact/max cases. NULL has zero-width Decimal storage.
                if matches!(case, Case::DecimalExact) {
                    offsets.extend([record + 15, record + 28, record + 31]);
                } else if matches!(case, Case::DecimalMax) {
                    offsets.extend([record + 6, record + 19, record + 22]);
                }
            } else {
                offsets.extend([
                    record + 2,  // Byte, including inactive NULL payload.
                    record + 3,  // Int16.
                    record + 25, // Single raw bits.
                    record + 29, // Double raw bits.
                    record + 37, // DATE raw bits.
                    record + 45, // Native GUID first component.
                    record + 49, // Native GUID second component.
                    record + 51, // Native GUID third component.
                    record + 53, // Canonical GUID tail.
                    record + 61, // Boolean/presence mask.
                ]);
            }
            // Unused high mask bits are inert framing bytes, not permission
            // to alter NULL interpretation or widen the declared record.
            let mut padding = plan(&source);
            padding.after[record + width - 1] ^= if decimal(case) { 0x20 } else { 0x04 };
            assert!(matches!(
                check(&padding, &source.values),
                Err(EditError::Preservation)
            ));
            controls += 1;

            let global = 4096 + short(&source.before, 4096 + 14);
            let owned = 3 * 4096 + short(&source.before, 3 * 4096 + 14 + 2 * 3);
            let hint = 3 * 4096 + short(&source.before, 3 * 4096 + 14 + 2 * 7);
            offsets.extend([global + 6, owned + 6, hint + 6]);
            for offset in offsets {
                let mut forged = plan(&source);
                forged.after[offset] ^= 1;
                assert!(
                    matches!(check(&forged, &source.values), Err(EditError::Preservation)),
                    "{} header {code} offset {offset}",
                    case.id()
                );
                controls += 1;
            }
            let mut extra = plan(&source);
            extra.after.extend_from_slice(&[0; 4096]);
            assert!(matches!(
                check(&extra, &source.values),
                Err(EditError::Preservation)
            ));
            controls += 1;
        }
    }
    // This finite count is derived from the independently selected controls,
    // not from any production serializer or staging constants.
    assert_eq!(controls, 272);
}
