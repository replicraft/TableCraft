//! Independent TCNI0005 literals and metered data-only validation.
use super::{NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates};
use crate::{
    EditError, FittedColumnUpdates, FixedScalarUpdate, InspectError, SlotScalarUpdate,
    transaction::intent::NativeIntentError,
};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, LegacyDateTime, Value},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000_000)
}

fn text(value: &str) -> Value {
    Value::Text(value.into())
}

fn original(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeFittedEditIntent, NativeIntentError> {
    let specifications = [
        (
            "Text",
            NativeFittedUpdates::Variable(vec![
                SlotScalarUpdate::new(2, Value::Null, text("")),
                SlotScalarUpdate::new(0, text("old"), text("é\0")),
            ]),
        ),
        (
            "Bin",
            NativeFittedUpdates::Variable(vec![
                SlotScalarUpdate::new(1, Value::Binary(vec![]), Value::Null),
                SlotScalarUpdate::new(0, Value::Binary(vec![0, 255]), Value::Binary(vec![3, 0, 4])),
            ]),
        ),
        (
            "Date",
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                0,
                Value::DateTime(LegacyDateTime::from_bits(0).unwrap()),
                Value::DateTime(LegacyDateTime::from_bits(0x8000_0000_0000_0000).unwrap()),
            )]),
        ),
        ("Empty", NativeFittedUpdates::Variable(vec![])),
        (
            "I32",
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                2,
                Value::Int32(i32::MIN),
                Value::Null,
            )]),
        ),
        (
            "Flag",
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                1,
                Value::Boolean(false),
                Value::Boolean(true),
            )]),
        ),
        (
            "Money",
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                0,
                Value::Currency(Currency::from_scaled(i64::MIN)),
                Value::Currency(Currency::from_scaled(i64::MAX)),
            )]),
        ),
        (
            "Large",
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                0,
                Value::Int64(i64::MAX),
                Value::Int64(i64::MIN),
            )]),
        ),
    ];
    let mut groups = Vec::new();
    for (column, updates) in specifications {
        groups.push(NativeFittedIntentGroup::new(
            column, updates, budget, cancelled,
        )?);
    }
    NativeFittedEditIntent::new("Items", groups, b"original", budget, cancelled)
}

fn literal_value(output: &mut Vec<u8>, tag: u8, bytes: &[u8]) {
    output.push(tag);
    output.extend_from_slice(&[0, 0, 0]);
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(bytes);
}

fn literal() -> Vec<u8> {
    // Entire wire is authored using literal independent offsets, tags and
    // widths. The source digest uses only the public documented hash law.
    type Update = (u64, u8, Vec<u8>, u8, Vec<u8>);
    type Group = (&'static str, u8, Vec<Update>);
    let groups: [Group; 8] = [
        (
            "Text",
            1,
            vec![
                (2, 0, vec![], 6, vec![]),
                (0, 6, b"old".to_vec(), 6, vec![0xc3, 0xa9, 0]),
            ],
        ),
        (
            "Bin",
            1,
            vec![
                (1, 7, vec![], 0, vec![]),
                (0, 7, vec![0, 255], 7, vec![3, 0, 4]),
            ],
        ),
        (
            "Date",
            0,
            vec![(0, 5, vec![0; 8], 5, vec![0, 0, 0, 0, 0, 0, 0, 128])],
        ),
        ("Empty", 1, vec![]),
        ("I32", 0, vec![(2, 2, vec![0, 0, 0, 128], 0, vec![])]),
        ("Flag", 0, vec![(1, 1, vec![0], 1, vec![1])]),
        (
            "Money",
            0,
            vec![(
                0,
                4,
                i64::MIN.to_le_bytes().to_vec(),
                4,
                i64::MAX.to_le_bytes().to_vec(),
            )],
        ),
        (
            "Large",
            0,
            vec![(
                0,
                3,
                i64::MAX.to_le_bytes().to_vec(),
                3,
                i64::MIN.to_le_bytes().to_vec(),
            )],
        ),
    ];
    let mut wire = Vec::from(*b"TCNI0005");
    wire.extend_from_slice(&587_u32.to_le_bytes());
    wire.extend_from_slice(&[5, 0, 0, 0]);
    wire.extend_from_slice(&8_u64.to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update(8_u64.to_le_bytes());
    digest.update(b"original");
    wire.extend_from_slice(&digest.finalize());
    for number in [5_u32, 8, 9, 0] {
        wire.extend_from_slice(&number.to_le_bytes());
    }
    wire.extend_from_slice(b"Items");
    for (name, family, updates) in groups {
        let total = 20
            + name.len()
            + updates
                .iter()
                .map(|cell| 28 + cell.2.len() + cell.4.len())
                .sum::<usize>();
        wire.extend_from_slice(&(total as u32).to_le_bytes());
        wire.extend_from_slice(&(name.len() as u32).to_le_bytes());
        wire.extend_from_slice(&(updates.len() as u32).to_le_bytes());
        wire.extend_from_slice(&[family, 0, 0, 0]);
        wire.extend_from_slice(&0_u32.to_le_bytes());
        wire.extend_from_slice(name.as_bytes());
        for (row, expected_tag, expected, replacement_tag, replacement) in updates {
            wire.extend_from_slice(
                &((28 + expected.len() + replacement.len()) as u32).to_le_bytes(),
            );
            wire.extend_from_slice(&row.to_le_bytes());
            literal_value(&mut wire, expected_tag, &expected);
            literal_value(&mut wire, replacement_tag, &replacement);
        }
    }
    assert_eq!(wire.len(), 587);
    wire
}

#[test]
fn literal_fitted_wire_keeps_kind_order_payloads_and_borrowed_request_identity() {
    let intent = original(&mut budget(), &|| false).unwrap();
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), literal());
    let decoded = NativeFittedEditIntent::decode(&literal(), &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.table(), "Items");
    assert_eq!(decoded.before_length(), 8);
    assert_eq!(decoded.groups().len(), 8);
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), literal());
    decoded
        .verify_before(b"original", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        decoded.verify_before(b"changed!", &mut budget(), &|| false),
        Err(NativeIntentError::BeforeMismatch)
    );
    let NativeFittedUpdates::Variable(updates) = decoded.groups()[0].updates() else {
        panic!()
    };
    assert_eq!(updates[0].expected(), &Value::Null);
    assert_eq!(updates[0].value(), &text(""));
    assert_eq!(updates[1].value(), &text("é\0"));
    let pointer = updates.as_ptr();
    let borrowed = decoded.borrowed_groups(&mut budget(), &|| false).unwrap();
    let FittedColumnUpdates::Variable { column, updates } = &borrowed[0] else {
        panic!()
    };
    assert_eq!(*column, "Text");
    assert_eq!(updates.as_ptr(), pointer);
    let FittedColumnUpdates::Variable { updates, .. } = &borrowed[3] else {
        panic!()
    };
    assert!(updates.is_empty());
    let NativeFittedUpdates::Fixed(dates) = decoded.groups()[2].updates() else {
        panic!()
    };
    assert_eq!(
        dates[0].expected(),
        &Value::DateTime(LegacyDateTime::from_bits(0).unwrap())
    );
    assert_eq!(
        dates[0].value(),
        &Value::DateTime(LegacyDateTime::from_bits(0x8000_0000_0000_0000).unwrap())
    );
}

fn no_allocation_decode_error(bytes: &[u8]) -> NativeIntentError {
    let mut allowance = ResourceBudget::new(0, 0, 1_000_000_000);
    let error = NativeFittedEditIntent::decode(bytes, &mut allowance, &|| false)
        .err()
        .expect("malformed data must refuse before decoded allocation");
    assert_eq!(allowance.remaining(Resource::Bytes), 0);
    assert!(!matches!(
        error,
        NativeIntentError::Inspect(InspectError::Limit(_))
    ));
    error
}

#[test]
fn framing_padding_unknown_tags_utf8_and_excess_refuse_before_allocation() {
    let original = literal();
    for end in 0..original.len() {
        no_allocation_decode_error(&original[..end]);
    }
    let mut trailing = original.clone();
    trailing.push(0);
    trailing[8..12].copy_from_slice(&588_u32.to_le_bytes());
    no_allocation_decode_error(&trailing);
    for at in [14, 68, 90, 93, 114] {
        let mut raw = original.clone();
        raw[at] = 1;
        assert_eq!(
            no_allocation_decode_error(&raw),
            NativeIntentError::InvalidRecord
        );
    }
    for (at, value) in [(89, 2), (113, 99), (160, 255), (439, 2), (72, 0)] {
        let mut raw = original.clone();
        raw[at] = value;
        assert_eq!(
            no_allocation_decode_error(&raw),
            NativeIntentError::InvalidRecord
        );
    }
    for (at, value) in [(60, 257), (64, 16385), (145, 8193)] {
        let mut raw = original.clone();
        raw[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
        assert_eq!(
            no_allocation_decode_error(&raw),
            NativeIntentError::Inspect(InspectError::StructureLimit)
        );
    }
    let mut row = original.clone();
    row[133..141].copy_from_slice(&16384_u64.to_le_bytes());
    assert_eq!(
        no_allocation_decode_error(&row),
        NativeIntentError::Inspect(InspectError::StructureLimit)
    );
    let mut date = original.clone();
    date[291..299].copy_from_slice(&0x7ff0_0000_0000_0000_u64.to_le_bytes());
    assert_eq!(
        no_allocation_decode_error(&date),
        NativeIntentError::InvalidRecord
    );
    let mut version = original.clone();
    version[7] = b'4';
    assert_eq!(
        no_allocation_decode_error(&version),
        NativeIntentError::UnsupportedVersion
    );
    let mut profile = original.clone();
    profile[12] = 4;
    assert_eq!(
        no_allocation_decode_error(&profile),
        NativeIntentError::UnsupportedProfile
    );
}

#[test]
fn duplicate_columns_rows_and_wrong_family_carriers_are_not_serialized_permission() {
    let mut rows = literal();
    rows[133..141].copy_from_slice(&2_u64.to_le_bytes());
    assert_eq!(
        no_allocation_decode_error(&rows),
        NativeIntentError::Request(EditError::Conflict)
    );
    let mut columns = literal();
    columns[267..271].copy_from_slice(b"tExT");
    assert_eq!(
        no_allocation_decode_error(&columns),
        NativeIntentError::Request(EditError::Conflict)
    );
    let mut family = literal();
    family[89] = 0;
    assert_eq!(
        no_allocation_decode_error(&family),
        NativeIntentError::InvalidRecord
    );
    let error = NativeFittedIntentGroup::new(
        "Bad",
        NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(0, Value::Null, Value::Int32(1))]),
        &mut budget(),
        &|| false,
    )
    .err()
    .expect("variable values have their own family");
    assert_eq!(error, NativeIntentError::InvalidRecord);
    let error = NativeFittedIntentGroup::new(
        "Bad",
        NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
            0,
            Value::Null,
            text(&"x".repeat(8193)),
        )]),
        &mut budget(),
        &|| false,
    )
    .err()
    .expect("payload ceiling applies to owned construction");
    assert_eq!(error, NativeIntentError::InvalidRecord);
    let error = NativeFittedIntentGroup::new(
        "Bad",
        NativeFittedUpdates::Variable(vec![
            SlotScalarUpdate::new(0, Value::Null, text("a")),
            SlotScalarUpdate::new(0, Value::Null, text("b")),
        ]),
        &mut budget(),
        &|| false,
    )
    .err()
    .expect("duplicate original rows refuse");
    assert_eq!(error, NativeIntentError::Request(EditError::Conflict));
}

#[derive(Clone, Copy)]
enum Operation {
    Construct,
    Encode,
    Decode,
    Borrow,
    Verify,
}

fn run(
    operation: Operation,
    intent: &NativeFittedEditIntent,
    wire: &[u8],
    budget: &mut ResourceBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<(), NativeIntentError> {
    match operation {
        Operation::Construct => {
            drop(original(budget, cancel)?);
        }
        Operation::Encode => {
            drop(intent.encode(budget, cancel)?);
        }
        Operation::Decode => {
            drop(NativeFittedEditIntent::decode(wire, budget, cancel)?);
        }
        Operation::Borrow => {
            drop(intent.borrowed_groups(budget, cancel)?);
        }
        Operation::Verify => {
            intent.verify_before(b"original", budget, cancel)?;
        }
    }
    Ok(())
}

#[test]
fn fitted_model_codec_borrowing_and_binding_pay_exact_work_and_cancel() {
    let intent = original(&mut budget(), &|| false).unwrap();
    let wire = literal();
    for operation in [
        Operation::Construct,
        Operation::Encode,
        Operation::Decode,
        Operation::Borrow,
        Operation::Verify,
    ] {
        let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
        let mut measured = budget();
        let initial = pools.map(|resource| measured.remaining(resource));
        run(operation, &intent, &wire, &mut measured, &|| false).unwrap();
        let costs: [u64; 3] =
            std::array::from_fn(|index| initial[index] - measured.remaining(pools[index]));
        assert_eq!(costs[1], 0);
        assert!(costs[2] > 0);
        let mut exact = ResourceBudget::new(costs[0], 0, costs[2]);
        run(operation, &intent, &wire, &mut exact, &|| false).unwrap();
        for resource in pools {
            assert_eq!(exact.remaining(resource), 0);
        }
        for (index, resource) in pools.into_iter().enumerate() {
            if costs[index] == 0 {
                continue;
            }
            let mut limits = costs;
            limits[index] -= 1;
            let error = run(
                operation,
                &intent,
                &wire,
                &mut ResourceBudget::new(limits[0], 0, limits[2]),
                &|| false,
            )
            .unwrap_err();
            assert!(
                matches!(error, NativeIntentError::Inspect(InspectError::Limit(error)) if error.resource == resource)
            );
        }
        let polls = Cell::new(0_usize);
        run(operation, &intent, &wire, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        let terminal = polls.get();
        assert!(terminal > 2);
        for stop in [1, terminal / 2, terminal] {
            polls.set(0);
            let error = run(operation, &intent, &wire, &mut budget(), &|| {
                polls.set(polls.get() + 1);
                polls.get() == stop
            })
            .unwrap_err();
            assert_eq!(error, NativeIntentError::Inspect(InspectError::Cancelled));
            assert_eq!(polls.get(), stop);
        }
    }
}

mod owned_growth;
