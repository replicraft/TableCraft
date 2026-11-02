//! Literal TCNA5 execution envelope; no native source, resolver or provider calls.
use super::{NativeClockInsertIntent, NativeIntentError};
use crate::{InsertFieldValue, InspectError, NativeOmittedInsertIntent, OmittedRowInsert};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExtendedDateTime, Value},
};
use tablecraft_expressions::functions::dates::clock::ClockReading;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

fn reading() -> ClockReading {
    ClockReading::new(ExtendedDateTime::new(2000, 2, 29, 12, 0, 0, 7_654_321).unwrap()).unwrap()
}

fn requests() -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(vec![
        InsertFieldValue::Supplied(Value::Int32(24)),
        InsertFieldValue::Omitted,
        InsertFieldValue::Supplied(Value::Null),
    ])]
}

fn literal(present: bool) -> Vec<u8> {
    // Independent frozen old presence grammar, including the original digest of b"original".
    let mut nested = Vec::from(*b"TCNA0002");
    nested.extend_from_slice(&112_u32.to_le_bytes());
    nested.extend_from_slice(&[2, 0, 0, 0]);
    nested.extend_from_slice(&8_u64.to_le_bytes());
    for pair in b"da0a5f300db10f6a7396b20b324a8f569ea50cdbc8416d136c45a933dc2e35f3"
        .as_chunks::<2>()
        .0
    {
        nested.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
    }
    for number in [8_u32, 1, 0] {
        nested.extend_from_slice(&number.to_le_bytes());
    }
    nested.extend_from_slice(b"Defaults");
    nested.extend_from_slice(&36_u32.to_le_bytes());
    nested.extend_from_slice(&3_u32.to_le_bytes());
    nested.extend_from_slice(&[2, 0, 0, 0, 4, 0, 0, 0, 24, 0, 0, 0]);
    nested.extend_from_slice(&[8, 0, 0, 0, 0, 0, 0, 0]);
    nested.extend_from_slice(&[0; 8]);
    assert_eq!(nested.len(), 112);
    let mut raw = Vec::from(*b"TCNA0005");
    raw.extend_from_slice(&144_u32.to_le_bytes());
    raw.extend_from_slice(&[5, 0, 0, 0]);
    if present {
        raw.extend_from_slice(&[1, 0, 0xd0, 7, 2, 29, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    } else {
        raw.extend_from_slice(&[0; 16]);
    }
    raw.extend_from_slice(&nested);
    assert_eq!(raw.len(), 144);
    raw
}

#[test]
fn captured_civil_envelope_matches_literal_and_keeps_the_old_presence_dialect_closed() {
    for present in [false, true] {
        let intent = NativeClockInsertIntent::new(
            "Defaults",
            requests(),
            b"original",
            present.then(reading),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let wanted = literal(present);
        assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wanted);
        let decoded = NativeClockInsertIntent::decode(&wanted, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.table(), "Defaults");
        assert_eq!(decoded.rows().len(), 1);
        assert_eq!(decoded.reading().is_some(), present);
        if let Some(value) = decoded.reading() {
            assert_eq!(value.date_time().date(), (2000, 2, 29));
            assert_eq!(value.date_time().time(), (12, 0, 0, 0));
        }
        decoded
            .verify_before(b"original", &mut budget(), &|| false)
            .unwrap();
        assert!(matches!(
            decoded.verify_before(&[0; 8], &mut budget(), &|| false),
            Err(NativeIntentError::BeforeMismatch)
        ));
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wanted);
        assert!(matches!(
            NativeOmittedInsertIntent::decode(&wanted, &mut budget(), &|| false),
            Err(NativeIntentError::UnsupportedVersion)
        ));
        assert!(matches!(
            NativeClockInsertIntent::decode(&wanted[32..], &mut budget(), &|| false),
            Err(NativeIntentError::UnsupportedVersion)
        ));
        for end in 0..wanted.len() {
            assert!(
                NativeClockInsertIntent::decode(&wanted[..end], &mut budget(), &|| false).is_err()
            );
        }
        for at in [14, 15, 17, 25, 26, 27, 28, 29, 30, 31] {
            let mut changed = wanted.clone();
            changed[at] = 1;
            assert!(NativeClockInsertIntent::decode(&changed, &mut budget(), &|| false).is_err());
        }
        let mut tail = wanted.clone();
        tail.push(0);
        tail[8..12].copy_from_slice(&145_u32.to_le_bytes());
        assert!(NativeClockInsertIntent::decode(&tail, &mut budget(), &|| false).is_err());
    }
    for (at, value) in [
        (16, 2),
        (18, 99),
        (20, 13),
        (21, 30),
        (22, 24),
        (23, 60),
        (24, 60),
    ] {
        let mut changed = literal(true);
        changed[at] = value;
        assert!(NativeClockInsertIntent::decode(&changed, &mut budget(), &|| false).is_err());
    }
    for year in [99_u16, 10_000] {
        let mut changed = literal(true);
        changed[18..20].copy_from_slice(&year.to_le_bytes());
        assert!(NativeClockInsertIntent::decode(&changed, &mut budget(), &|| false).is_err());
    }
    let mut absent_with_civil = literal(false);
    absent_with_civil[20] = 1;
    assert!(NativeClockInsertIntent::decode(&absent_with_civil, &mut budget(), &|| false).is_err());
}

#[test]
fn complete_envelope_decode_pays_exact_pools_and_polls_entry_middle_terminal_cancellation() {
    let raw = literal(true);
    let mut measured = budget();
    let initial = [Resource::Bytes, Resource::Steps].map(|resource| measured.remaining(resource));
    let polls = Cell::new(0_usize);
    NativeClockInsertIntent::decode(&raw, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    for (at, resource) in [Resource::Bytes, Resource::Steps].into_iter().enumerate() {
        let used = initial[at] - measured.remaining(resource);
        assert!(used > 0);
        let mut exact = budget();
        exact
            .charge(resource, exact.remaining(resource) - used)
            .unwrap();
        NativeClockInsertIntent::decode(&raw, &mut exact, &|| false).unwrap();
        assert_eq!(exact.remaining(resource), 0);
        let mut short = budget();
        short
            .charge(resource, short.remaining(resource) - used + 1)
            .unwrap();
        assert!(
            matches!(NativeClockInsertIntent::decode(&raw, &mut short, &|| false), Err(NativeIntentError::Inspect(InspectError::Limit(error))) if error.resource == resource)
        );
    }
    let terminal = polls.get();
    assert!(terminal > 2);
    for stop in [1, terminal.div_ceil(2), terminal] {
        let seen = Cell::new(0_usize);
        assert!(matches!(
            NativeClockInsertIntent::decode(&raw, &mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            }),
            Err(NativeIntentError::Inspect(InspectError::Cancelled))
        ));
        assert_eq!(seen.get(), stop);
    }
}
