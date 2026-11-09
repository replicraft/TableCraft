use super::*;
use crate::Header;
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_testkit::fixtures::access::canonical_created_number::{Source, repeated};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

#[test]
fn visitor_retains_real_native_number_id_and_name_span_with_only_its_paid_work() {
    let mut bytes = repeated::source(Source::IdOnly);
    // Independent creator framing: one 12-byte index, two 25-byte fields;
    // the second field's stored ID is deliberately different from number 1.
    let stored_id = 7 * 4096 + 63 + 12 + 25 + 9;
    bytes[stored_id..stored_id + 2].copy_from_slice(&17_u16.to_le_bytes());
    let header = Header::parse(&bytes, bytes.len() as u64).unwrap();
    let mut ordinary = budget();
    let plain = read(
        &mut Reader {
            source: &mut Cursor::new(&bytes),
            header,
            budget: &mut ordinary,
            cancelled: &|| false,
            reads: 0,
        },
        7,
    )
    .unwrap();
    let mut observed = budget();
    let mut selected = None;
    let parsed = read_with_field_names(
        &mut Reader {
            source: &mut Cursor::new(&bytes),
            header,
            budget: &mut observed,
            cancelled: &|| false,
            reads: 0,
        },
        7,
        |number, id, entry, name| {
            if number == 1 {
                selected = Some((id, entry, name));
            }
            Ok(())
        },
    )
    .unwrap();
    assert!(plain == parsed);
    let (id, entry, name) = selected.unwrap();
    assert_eq!(id, 17);
    assert_eq!(entry, 100..125);
    assert_eq!(name, 133..143);
    assert_eq!(
        &bytes[7 * 4096 + name.start..7 * 4096 + name.end],
        b"S\0c\0o\0r\0e\0"
    );
    assert_eq!(
        ordinary.remaining(Resource::Bytes),
        observed.remaining(Resource::Bytes)
    );
    assert_eq!(
        ordinary.remaining(Resource::Steps) - observed.remaining(Resource::Steps),
        4
    );
}

#[test]
fn a_later_definition_error_never_publishes_a_partial_layout() {
    let first = repeated::source(Source::IdOnly);
    let mut bytes = repeated::append(&first, "Score2");
    let third_number = 7 * 4096 + 63 + 12 + 2 * 25 + 5;
    bytes[third_number..third_number + 2].copy_from_slice(&0_u16.to_le_bytes());
    let header = Header::parse(&bytes, bytes.len() as u64).unwrap();
    let mut visited = false;
    let result = read_with_field_names(
        &mut Reader {
            source: &mut Cursor::new(&bytes),
            header,
            budget: &mut budget(),
            cancelled: &|| false,
            reads: 0,
        },
        7,
        |number, _, _, _| {
            visited |= number == 1;
            Ok(())
        },
    );
    assert!(visited);
    assert!(matches!(result, Err(InspectError::Malformed)));
}
