use super::Presentation;
use crate::Error;
use tablecraft_core::limits::{Resource, ResourceBudget};

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
