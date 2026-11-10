//! Retained protocol data never grants the older request families new authority.
use super::{NativeIntentError, NativeOmittedInsertIntent};
use crate::{InsertFieldValue, OmittedRowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_testkit::fixtures::access::tcna2;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 500_000_000)
}

fn requests() -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(vec![
        InsertFieldValue::Omitted,
        InsertFieldValue::Supplied(Value::Null),
        InsertFieldValue::Supplied(Value::Text("日本語🦀".into())),
    ])]
}

fn literal(before: &[u8], table: &str, rows: Vec<Vec<Option<Value>>>) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut wire = tcna2::wider_presence_wire(table, before.len(), digest.finalize().into(), rows);
    wire[..8].copy_from_slice(b"TCNA0008");
    wire[12..14].copy_from_slice(&8_u16.to_le_bytes());
    wire
}

#[test]
fn retained_presence_matches_independent_wire_and_older_decoders_remain_closed() {
    // An inert binding is not source qualification or permission to save.
    let before = [0xa5; 4096];
    let wire = literal(
        &before,
        "Defaults",
        vec![vec![
            None,
            Some(Value::Null),
            Some(Value::Text("日本語🦀".into())),
        ]],
    );
    let intent = NativeOmittedInsertIntent::new_retained(
        "Defaults",
        requests(),
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
    let decoded =
        NativeOmittedInsertIntent::decode_retained(&wire, &mut budget(), &|| false).unwrap();
    assert!(decoded.is_retained());
    assert!(matches!(
        decoded.rows()[0].values()[0],
        InsertFieldValue::Omitted
    ));
    assert!(matches!(
        decoded.rows()[0].values()[1],
        InsertFieldValue::Supplied(Value::Null)
    ));
    assert!(
        matches!(&decoded.rows()[0].values()[2], InsertFieldValue::Supplied(Value::Text(text)) if text == "日本語🦀")
    );
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
    assert!(NativeOmittedInsertIntent::decode(&wire, &mut budget(), &|| false).is_err());
    assert!(NativeOmittedInsertIntent::decode_current(&wire, &mut budget(), &|| false).is_err());

    for version in [2_u8, 6, 7] {
        let mut older = wire.to_vec();
        older[7] = b'0' + version;
        older[12] = version;
        assert!(
            NativeOmittedInsertIntent::decode_retained(&older, &mut budget(), &|| false).is_err()
        );
    }
    let mut changed = before;
    changed[100] ^= 1;
    assert!(matches!(
        decoded.verify_before(&changed, &mut budget(), &|| false),
        Err(NativeIntentError::BeforeMismatch)
    ));
}

#[test]
fn retained_preflight_rejects_truncation_framing_and_explicit_budget_exhaustion() {
    let wire = literal(&[0; 4096], "Defaults", vec![vec![None, Some(Value::Null)]]);
    for end in 0..wire.len() {
        let mut work = budget();
        let bytes = work.remaining(Resource::Bytes);
        assert!(
            NativeOmittedInsertIntent::decode_retained(&wire[..end], &mut work, &|| false).is_err()
        );
        assert_eq!(work.remaining(Resource::Bytes), bytes);
    }
    for (offset, value) in [(8, 0), (12, 7), (14, 1), (64, 1)] {
        let mut malformed = wire.to_vec();
        malformed[offset] = value;
        let mut work = budget();
        let bytes = work.remaining(Resource::Bytes);
        assert!(
            NativeOmittedInsertIntent::decode_retained(&malformed, &mut work, &|| false).is_err()
        );
        assert_eq!(work.remaining(Resource::Bytes), bytes);
    }
    for mut work in [
        ResourceBudget::new(0, 0, 500_000_000),
        ResourceBudget::new(512 * 1024 * 1024, 0, 0),
    ] {
        assert!(NativeOmittedInsertIntent::decode_retained(&wire, &mut work, &|| false).is_err());
    }
    assert!(NativeOmittedInsertIntent::decode_retained(&wire, &mut budget(), &|| true).is_err());
}

#[test]
fn retained_binding_and_request_data_do_not_inherit_old_capture_or_carrier_quotas() {
    // This deliberately is not a database image. Only inert request framing is tested.
    let before = vec![0; 64 * 1024 * 1024 + 4096];
    let rows = vec![OmittedRowInsert::new(vec![InsertFieldValue::Supplied(
        Value::Text("x".repeat(8193)),
    )])];
    let intent =
        NativeOmittedInsertIntent::new_retained("Defaults", rows, &before, &mut budget(), &|| {
            false
        })
        .unwrap();
    let wire = intent.encode(&mut budget(), &|| false).unwrap();
    let decoded =
        NativeOmittedInsertIntent::decode_retained(&wire, &mut budget(), &|| false).unwrap();
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    assert!(
        matches!(&decoded.rows()[0].values()[0], InsertFieldValue::Supplied(Value::Text(text)) if text.len() == 8193)
    );
    assert!(
        NativeOmittedInsertIntent::new_current(
            "Defaults",
            Vec::new(),
            &before,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert!(
        NativeOmittedInsertIntent::new_retained(
            "Defaults",
            Vec::new(),
            &before[..4095],
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[cfg(feature = "ace16-create")]
mod recovery;
