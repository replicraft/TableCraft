//! Predicate-only GUID-structure bytes; strict relational keys keep their carriers.
use crate::{EvalError, comparison::Comparison, logic::Truth};
use tablecraft_core::values::{Guid, Value};
use uuid::Uuid;

pub(super) fn compare(
    operator: Comparison,
    left: &Value,
    right: &Value,
) -> Result<Option<Truth>, EvalError> {
    let (guid, bytes) = match (left, right) {
        (Value::Guid(guid), Value::Binary(bytes)) | (Value::Binary(bytes), Value::Guid(guid)) => {
            (guid, bytes)
        }
        _ => return Ok(None),
    };
    let bytes: &[u8; Guid::BYTE_LEN] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| EvalError::TypeMismatch)?;
    let expected = Uuid::from_bytes(guid.to_bytes()).to_bytes_le();
    let equal = match operator {
        Comparison::Equal => expected == *bytes,
        Comparison::NotEqual => expected != *bytes,
        _ => return Err(EvalError::UnsupportedExpression),
    };
    Ok(Some(if equal { Truth::True } else { Truth::False }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        comparison,
        evaluation::{EvalLimits, Evaluator, NoBindings},
    };
    use proptest::prelude::*;
    use tablecraft_core::limits::ResourceBudget;

    fn guid() -> Value {
        Value::Guid(Guid::from_bytes([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ]))
    }

    fn bytes() -> Value {
        Value::Binary(vec![
            0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ])
    }

    #[test]
    fn contextual_equality_uses_structure_order_without_changing_strict_keys() {
        let mut budget = ResourceBudget::new(0, 0, 100);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        for (left, right) in [(guid(), bytes()), (bytes(), guid())] {
            assert_eq!(
                evaluator.compare_values(Comparison::Equal, &left, &right),
                Ok(Truth::True)
            );
            assert_eq!(
                evaluator.compare_effective_numeric_values(Comparison::NotEqual, &left, &right),
                Ok(Truth::False)
            );
            assert_eq!(
                comparison::compare(Comparison::Equal, &left, &right, None),
                Err(EvalError::TypeMismatch)
            );
        }
        let swapped = Value::Binary(match guid() {
            Value::Guid(value) => value.to_bytes().to_vec(),
            _ => unreachable!(),
        });
        assert_eq!(
            evaluator.compare_values(Comparison::Equal, &guid(), &swapped),
            Ok(Truth::False)
        );
    }

    #[test]
    fn widths_unrelated_binary_ordering_and_null_retain_explicit_boundaries() {
        let mut budget = ResourceBudget::new(0, 0, 1000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        for width in 0..=32 {
            if width == 16 {
                continue;
            }
            let malformed = Value::Binary(vec![0; width]);
            for (left, right) in [(guid(), malformed.clone()), (malformed, guid())] {
                assert_eq!(
                    evaluator.compare_values(Comparison::Equal, &left, &right),
                    Err(EvalError::TypeMismatch)
                );
            }
        }
        for operator in [
            Comparison::Less,
            Comparison::LessOrEqual,
            Comparison::Greater,
            Comparison::GreaterOrEqual,
        ] {
            assert_eq!(
                evaluator.compare_values(operator, &guid(), &bytes()),
                Err(EvalError::UnsupportedExpression)
            );
        }
        assert_eq!(
            evaluator.compare_values(Comparison::Equal, &bytes(), &bytes()),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            evaluator.compare_values(
                Comparison::Equal,
                &guid(),
                &Value::Text("00112233-4455-6677-8899-aabbccddeeff".into())
            ),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            evaluator.compare_values(Comparison::Equal, &Value::Null, &Value::Binary(vec![])),
            Ok(Truth::Unknown)
        );
    }

    #[test]
    fn cancellation_and_work_refusals_precede_guid_coercion() {
        let mut budget = ResourceBudget::new(0, 0, 0);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        assert!(matches!(
            evaluator.compare_values(Comparison::Equal, &guid(), &bytes()),
            Err(EvalError::Limit(_))
        ));
        let mut budget = ResourceBudget::new(0, 0, 10);
        let cancelled = || true;
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&cancelled);
        assert_eq!(
            evaluator.compare_values(Comparison::Equal, &guid(), &bytes()),
            Err(EvalError::Cancelled)
        );
    }

    proptest! {
        #[test]
        fn every_canonical_pattern_compares_to_independent_group_reversal(
            canonical in any::<[u8; 16]>(),
            bit in 0usize..128,
        ) {
            let mut structure = canonical;
            structure[..4].reverse();
            structure[4..6].reverse();
            structure[6..8].reverse();
            let guid = Value::Guid(Guid::from_bytes(canonical));
            let matching = Value::Binary(structure.to_vec());
            prop_assert_eq!(compare(Comparison::Equal, &guid, &matching), Ok(Some(Truth::True)));
            structure[bit / 8] ^= 1 << (bit % 8);
            let different = Value::Binary(structure.to_vec());
            prop_assert_eq!(compare(Comparison::NotEqual, &different, &guid), Ok(Some(Truth::True)));
        }
    }
}
