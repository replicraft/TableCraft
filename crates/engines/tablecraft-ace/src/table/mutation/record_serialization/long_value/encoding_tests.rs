//! Writer payload admission follows descriptor geometry and explicit resources.
#[cfg(feature = "ace16-write-preview")]
use super::StorageChoice;
use super::{Encoded, encode};
use crate::{
    EditError, InspectError, NativeColumn, inspect_table_schema,
    table::mutation::{
        record_serialization::proof::long_value::{self as proof, Payload},
        value_limits::require_encoded_length,
    },
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::mutation_fixture;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 100_000_000)
}

fn field(kind: u8, compressed: bool) -> NativeColumn {
    let mut schema = inspect_table_schema(
        &mut Cursor::new(mutation_fixture::database()),
        CatalogScope::new(810),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut field = schema.columns.remove(1);
    field.field.kind = kind;
    field.field.fixed = false;
    field.field.flags = 2;
    field.field.extended_flags = u8::from(compressed);
    field.field.length = 0;
    field
}

#[test]
fn writer_memo_exceeds_old_text_and_encoded_caps_with_independent_reconstruction() {
    for text in ["é".repeat(524_289), "🦀".repeat(524_289)] {
        let selected = field(12, true);
        let value = Value::Text(text);
        let Encoded::Present(actual) = encode(&selected, &value, &mut budget(), &|| false).unwrap()
        else {
            panic!("present Memo must retain its encoded bytes");
        };
        let Payload::Present(expected) =
            proof::requested(&selected, &value, &mut budget(), &|| false).unwrap()
        else {
            panic!("independent request proof must retain the Memo bytes");
        };
        assert_eq!(actual, expected);
        let Value::Text(text) = &value else {
            unreachable!()
        };
        assert!(text.len() > 1_048_576);
        assert_eq!(actual.len(), text.encode_utf16().count() * 2);
        assert!(
            actual
                .iter()
                .copied()
                .eq(text.encode_utf16().flat_map(u16::to_le_bytes))
        );
        if text.starts_with('🦀') {
            assert!(actual.len() > 2_097_152);
        }
        for mut allowance in [
            ResourceBudget::new(0, 100_000, 100_000_000),
            ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
        ] {
            assert!(matches!(
                encode(&selected, &value, &mut allowance, &|| false),
                Err(EditError::Inspect(InspectError::Limit(_)))
            ));
        }
        assert!(matches!(
            proof::requested(&selected, &value, &mut budget(), &|| true),
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
    }
}

#[test]
fn writer_ole_exceeds_old_payload_cap_with_exact_request_bytes() {
    let selected = field(11, false);
    let value = Value::Binary((0..2_097_153).map(|at| (at % 251) as u8).collect());
    let Encoded::Present(actual) = encode(&selected, &value, &mut budget(), &|| false).unwrap()
    else {
        panic!("present OLE must retain its encoded bytes");
    };
    let Payload::Present(expected) =
        proof::requested(&selected, &value, &mut budget(), &|| false).unwrap()
    else {
        panic!("independent request proof must retain OLE bytes");
    };
    assert_eq!(actual.len(), 2_097_153);
    assert_eq!(actual, expected);
    assert!(
        actual
            .iter()
            .enumerate()
            .all(|(at, byte)| *byte == (at % 251) as u8)
    );
}

#[test]
fn writer_value_lengths_use_all_thirty_descriptor_bits_without_allocating_the_limit() {
    assert!(require_encoded_length(0x3fff_ffff).is_ok());
    assert!(matches!(
        require_encoded_length(0x4000_0000),
        Err(InspectError::StructureLimit)
    ));
}

#[cfg(feature = "ace16-write-preview")]
#[test]
fn writer_chunk_plan_exceeds_1024_using_actual_payload_geometry() {
    let encoded = Encoded::Present(vec![0x5a; 4072 * 1024 + 1]);
    let chunks = encoded
        .chunks(StorageChoice::Chained, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(chunks.len(), 1025);
    assert_eq!(chunks.first().unwrap().payload, 0..4072);
    assert_eq!(chunks.last().unwrap().payload, 4_169_728..4_169_729);
    for pair in chunks.windows(2) {
        assert_eq!(pair[0].payload.end, pair[1].payload.start);
    }
    let descriptor = encoded
        .descriptor(
            StorageChoice::Chained,
            Some(0x10000),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(descriptor, [1, 160, 63, 0, 0, 0, 1, 0, 0, 0, 0, 0]);
    assert_eq!(
        encoded
            .body(chunks.last().unwrap(), 0, &mut budget(), &|| false)
            .unwrap(),
        [0, 0, 0, 0, 0x5a]
    );
    assert!(matches!(
        encoded.chunks(
            StorageChoice::Chained,
            &mut ResourceBudget::new(0, 1, 100_000),
            &|| false
        ),
        Err(EditError::Inspect(InspectError::Limit(_)))
    ));
    assert!(matches!(
        encoded.chunks(StorageChoice::Chained, &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
}
