use super::Presentation;
use crate::{Cell, CellKind, Error};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

#[test]
fn column_envelopes_and_header_payloads_share_the_bounded_output_pool() {
    let envelope = 2 * size_of::<String>() as u64;
    let mut exact = Presentation {
        work: ResourceBudget::new(envelope + 3, 0, 10),
        cancelled: &|| false,
    };
    let mut columns = exact.columns(2).unwrap();
    columns.push(exact.text("Id").unwrap());
    columns.push(exact.text("X").unwrap());
    assert_eq!(columns, ["Id", "X"]);
    assert_eq!(exact.work.remaining(Resource::Bytes), 0);

    let mut missing_envelope_byte = Presentation {
        work: ResourceBudget::new(envelope - 1, 0, 10),
        cancelled: &|| false,
    };
    assert!(
        matches!(missing_envelope_byte.columns(2), Err(Error::Limit(limit))
        if limit.resource == Resource::Bytes && limit.remaining == envelope - 1)
    );
    assert_eq!(
        missing_envelope_byte.work.remaining(Resource::Bytes),
        envelope - 1
    );

    let mut missing_payload_byte = Presentation {
        work: ResourceBudget::new(envelope + 2, 0, 10),
        cancelled: &|| false,
    };
    let mut columns = missing_payload_byte.columns(2).unwrap();
    columns.push(missing_payload_byte.text("Id").unwrap());
    assert!(
        matches!(missing_payload_byte.text("X"), Err(Error::Limit(limit))
        if limit.resource == Resource::Bytes && limit.remaining == 0)
    );
    assert_eq!(columns, ["Id"]);
}

#[test]
fn boolean_facts_are_independent_of_cached_provenance_and_expression_text() {
    let mut output = Presentation::new(&|| false);
    for (value, boolean, is_null) in [
        (Value::Boolean(true), Some(true), false),
        (Value::Boolean(false), Some(false), false),
        (Value::Null, None, true),
        (Value::Text("true".into()), None, false),
        (Value::Int32(-1), None, false),
    ] {
        let cell = output.scalar(&value, true).unwrap();
        assert_eq!(
            cell.kind,
            CellKind::Scalar {
                is_null,
                stored_calculation: true,
                boolean,
                display_control: crate::DisplayControl::Uninspected,
            }
        );
    }
    let expression = Cell::expression("true".into());
    assert_eq!(
        expression.kind,
        CellKind::Scalar {
            is_null: false,
            stored_calculation: false,
            boolean: None,
            display_control: crate::DisplayControl::Uninspected
        }
    );
}
