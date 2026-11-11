//! Preservation comparisons consume work without allocating or coercing values.
use super::{equal_payload, equal_value};
use crate::{CurrentFieldAdditionError as Error, EditError, InspectError};
use std::cell::Cell;
use tablecraft_core::{
    limits::{LimitError, Resource, ResourceBudget},
    values::{Double, Value},
};

#[test]
fn equal_large_sibling_and_property_payloads_exhaust_work_before_claiming_preservation() {
    let left = Value::Text("雪".repeat(16_384));
    let right = Value::Text("雪".repeat(16_384));
    let mut scalar_work = ResourceBudget::new(0, 0, 16);
    assert!(
        equal_value(
            &Value::Int64(9_007_199_254_740_993),
            &Value::Int64(9_007_199_254_740_993),
            &mut scalar_work,
            &|| false
        )
        .unwrap()
    );
    assert!(matches!(
        equal_value(&left, &right, &mut ResourceBudget::new(0, 0, 16), &|| false),
        Err(Error::Native(EditError::Inspect(InspectError::Limit(
            LimitError {
                resource: Resource::Steps,
                ..
            }
        ))))
    ));
    assert!(
        equal_value(
            &left,
            &right,
            &mut ResourceBudget::new(0, 0, u64::MAX),
            &|| false
        )
        .unwrap()
    );
    let body = vec![0x92; 16_384];
    assert!(matches!(
        equal_payload(&body, &body, &mut ResourceBudget::new(0, 0, 16), &|| false),
        Err(Error::Native(EditError::Inspect(InspectError::Limit(
            LimitError {
                resource: Resource::Steps,
                ..
            }
        ))))
    ));
    assert!(
        equal_payload(
            &body,
            &body,
            &mut ResourceBudget::new(0, 0, u64::MAX),
            &|| false
        )
        .unwrap()
    );
}

#[test]
fn long_preservation_comparisons_check_cancellation_after_entry_and_keep_exact_carriers() {
    let payload = Value::Binary(vec![0x92; 16_384]);
    let checks = Cell::new(0);
    let cancelled = || {
        let count = checks.get() + 1;
        checks.set(count);
        count >= 4
    };
    assert!(matches!(
        equal_value(
            &payload,
            &payload,
            &mut ResourceBudget::new(0, 0, u64::MAX),
            &cancelled
        ),
        Err(Error::Native(EditError::Inspect(InspectError::Cancelled)))
    ));
    assert!(
        checks.get() > 1,
        "cancellation must be observed during payload comparison"
    );
    let mut work = ResourceBudget::new(0, 0, u64::MAX);
    assert!(
        !equal_value(
            &Value::Double(Double::from_bits(0)),
            &Value::Double(Double::from_bits(0x8000_0000_0000_0000)),
            &mut work,
            &|| false
        )
        .unwrap()
    );
    assert!(
        !equal_value(
            &Value::Null,
            &Value::Text(String::new()),
            &mut work,
            &|| false
        )
        .unwrap()
    );
    assert!(!equal_payload(b"Required", b"required", &mut work, &|| false).unwrap());
}
