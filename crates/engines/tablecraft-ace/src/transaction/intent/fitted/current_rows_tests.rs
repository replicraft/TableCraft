//! Independent original TCNI0016 bytes and cold current-row reconstruction.
use crate::{
    FixedScalarUpdate, NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates,
    NativeImage, NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
    SlotScalarUpdate, qualify_native_update_recovery,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::journal;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::mixed::after_off::deleted_field::populated::{
        self as raw, readdition,
    },
    support::io::journal::MemoryStorage,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn groups(expected: Option<&str>, value: Option<&str>) -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new(
            "NextText",
            NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
                0,
                expected.map_or(Value::Null, |text| Value::Text(text.into())),
                value.map_or(Value::Null, |text| Value::Text(text.into())),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn prefix(before: &[u8], length: u32) -> Vec<u8> {
    let mut wire = b"TCNI0016".to_vec();
    wire.extend(length.to_le_bytes());
    wire.extend([16, 0, 0, 0]);
    wire.extend((before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    wire.extend(digest.finalize());
    for number in [6_u32, 1, 1, 0] {
        wire.extend(number.to_le_bytes());
    }
    wire.extend(b"Table1");
    wire
}

fn literal(before: &[u8], expected: Option<&str>, value: Option<&str>) -> Vec<u8> {
    let payload = expected.map_or(0, str::len) + value.map_or(0, str::len);
    let mut wire = prefix(before, 134 + payload as u32);
    wire.extend((56_u32 + payload as u32).to_le_bytes());
    wire.extend(8_u32.to_le_bytes());
    wire.extend(1_u32.to_le_bytes());
    wire.extend([1, 0, 0, 0]);
    wire.extend([0; 4]);
    wire.extend(b"NextText");
    wire.extend((28_u32 + payload as u32).to_le_bytes());
    wire.extend([0; 8]);
    for cell in [expected, value] {
        wire.extend([if cell.is_some() { 6 } else { 0 }, 0, 0, 0]);
        wire.extend((cell.map_or(0, str::len) as u32).to_le_bytes());
        if let Some(text) = cell {
            wire.extend(text.as_bytes());
        }
    }
    assert_eq!(wire.len(), 134 + payload);
    wire
}

#[test]
fn current_row_intent_binds_distinct_literal_wire_and_fresh_reconstruction() {
    for case in raw::CASES {
        let before = readdition::after(&readdition::source(case), readdition::Target::ShortText);
        for text in ["", "replacement", "日本語🦀"] {
            let after = readdition::fill_text(case, &before, 0, text);
            let plan = image(&before, 201)
                .plan_recoverable_current_row_updates(
                    "Table1",
                    groups(None, Some(text)),
                    &mut budget(),
                    &|| false,
                )
                .unwrap_or_else(|error| panic!("{case:?}/{text:?}: {error:?}"));
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16CurrentRows
            );
            assert_eq!(plan.after_bytes(), after, "{case:?}/{text:?}");
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(wire, literal(&before, None, Some(text)));
            assert!(NativeFittedEditIntent::decode(&wire, &mut budget(), &|| false).is_err());
            let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            assert_eq!(decoded.profile(), NativeUpdateProfile::Ace16CurrentRows);
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
            assert_eq!(
                plan.requalify_with_budgets(
                    image(&before, 202),
                    &mut budget(),
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .after_bytes(),
                after
            );
            for (magic, profile) in [
                (b"TCNI0005", 5_u16),
                (b"TCNI0006", 6),
                (b"TCNI0007", 7),
                (b"TCNI0015", 15),
            ] {
                let mut old = literal(&before, None, Some(text));
                old[..8].copy_from_slice(magic);
                old[12..14].copy_from_slice(&profile.to_le_bytes());
                let intent = NativeUpdateIntent::decode(&old, &mut budget(), &|| false).unwrap();
                assert!(
                    crate::transaction::update_rebuild::rebuild(
                        image(&before, 203),
                        &intent,
                        &mut budget(),
                        &|| false
                    )
                    .is_err(),
                    "{case:?} legacy profile {profile}"
                );
            }
        }
    }
}

#[test]
fn current_row_cold_recovery_reconstructs_first_and_present_values_and_refuses_forgery() {
    for case in raw::CASES {
        let absent = readdition::after(&readdition::source(case), readdition::Target::ShortText);
        for present in [false, true] {
            let before = if present {
                readdition::fill_text(case, &absent, 0, "old")
            } else {
                absent.to_vec()
            };
            let expected = present.then_some("old");
            let after = if present {
                readdition::replace_text(&before, 0, Some("saved 日本語"))
            } else {
                readdition::fill_text(case, &before, 0, "saved 日本語")
            };
            for committed in [false, true] {
                let mut target = MemoryStorage::new(before.to_vec());
                let mut log = MemoryStorage::new(Vec::new());
                let plan = image(&before, 204)
                    .plan_recoverable_current_row_updates(
                        "Table1",
                        groups(expected, Some("saved 日本語")),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(plan.after_bytes(), after);
                let prepared = plan
                    .prepare(&mut target, &mut log, &mut budget(), &|| false)
                    .unwrap();
                if committed {
                    prepared.commit(&mut budget(), &|| false).unwrap();
                } else {
                    drop(prepared);
                    target.bytes.get_mut()[8 * 4096..9 * 4096]
                        .copy_from_slice(&after[8 * 4096..9 * 4096]);
                }
                let checked = journal::inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
                let intent = NativeUpdateIntent::decode(
                    &literal(&before, expected, Some("saved 日本語")),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                qualify_native_update_recovery(
                    checked,
                    intent,
                    CatalogScope::new(205),
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap();
                assert_eq!(
                    target.bytes.get_ref(),
                    if committed { &after } else { &before }
                );
            }
            let mut forged = after.to_vec();
            *forged.last_mut().unwrap() ^= 1;
            let mut target = MemoryStorage::new(before.to_vec());
            let mut log = MemoryStorage::new(Vec::new());
            drop(
                journal::prepare_expected(
                    &mut target,
                    &mut log,
                    &before,
                    &forged,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
            let checked = journal::inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
            let intent = NativeUpdateIntent::decode(
                &literal(&before, expected, Some("saved 日本語")),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(matches!(
                qualify_native_update_recovery(
                    checked,
                    intent,
                    CatalogScope::new(206),
                    &mut budget(),
                    &|| false
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(target.writes, 0);
            assert!(
                image(&before, 207)
                    .plan_recoverable_current_row_updates(
                        "Table1",
                        groups(Some("stale"), Some("saved 日本語")),
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
        }
    }
}

fn number_groups(expected: Option<i32>, value: Option<i32>) -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new(
            "NextNumber",
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                0,
                expected.map_or(Value::Null, Value::Int32),
                value.map_or(Value::Null, Value::Int32),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn number_literal(before: &[u8], expected: Option<i32>, value: Option<i32>) -> Vec<u8> {
    assert_eq!(
        usize::from(expected.is_some()) + usize::from(value.is_some()),
        1
    );
    let mut wire = prefix(before, 140);
    wire.extend(62_u32.to_le_bytes());
    wire.extend(10_u32.to_le_bytes());
    wire.extend(1_u32.to_le_bytes());
    wire.extend([0; 8]);
    wire.extend(b"NextNumber");
    wire.extend(32_u32.to_le_bytes());
    wire.extend([0; 8]);
    for cell in [expected, value] {
        wire.extend([if cell.is_some() { 2 } else { 0 }, 0, 0, 0]);
        wire.extend((if cell.is_some() { 4_u32 } else { 0 }).to_le_bytes());
        if let Some(number) = cell {
            wire.extend(number.to_le_bytes());
        }
    }
    assert_eq!(wire.len(), 140);
    wire
}

// Independent DATA directory/pointer arithmetic locates the selected terminal;
// clearing a fixed NULL bit retains its ignored Number payload byte-for-byte.
fn cleared_number(before: &[u8]) -> Vec<u8> {
    let data = 8 * 4096;
    let word = |at: usize| usize::from(u16::from_le_bytes(before[at..at + 2].try_into().unwrap()));
    let root = word(data + 14);
    let physical = if root & 0x4000 != 0 {
        let pointer = data + (root & 0x1fff);
        let reference = u32::from_le_bytes(before[pointer..pointer + 4].try_into().unwrap());
        assert_eq!(reference >> 8, 8);
        reference as usize & 255
    } else {
        0
    };
    let start = word(data + 14 + physical * 2) & 0x1fff;
    let end = if physical == 0 {
        4096
    } else {
        word(data + 12 + physical * 2) & 0x1fff
    };
    let count = word(data + start);
    let number = word(7 * 4096 + 41) - 1;
    assert!(number < count);
    let mut after = before.to_vec();
    after[data + end - count.div_ceil(8) + number / 8] &= !(1 << (number % 8));
    after
}

#[test]
fn current_row_fixed_number_literal_keeps_null_and_int32_distinct() {
    for case in raw::CASES {
        let absent = readdition::after(&readdition::source(case), readdition::Target::Number);
        let supplied = readdition::fill_number(case, &absent, 0, 123);
        for (before, after, expected, value) in [
            (&absent, supplied.to_vec(), None, Some(123)),
            (&supplied, cleared_number(&supplied), Some(123), None),
        ] {
            let plan = image(before, 211)
                .plan_recoverable_current_row_updates(
                    "Table1",
                    number_groups(expected, value),
                    &mut budget(),
                    &|| false,
                )
                .unwrap_or_else(|error| panic!("{case:?}/{expected:?}->{value:?}: {error:?}"));
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16CurrentRows
            );
            assert_eq!(
                plan.after_bytes(),
                after,
                "{case:?}/{expected:?}->{value:?}"
            );
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(wire, number_literal(before, expected, value));
            let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
            assert_eq!(
                plan.requalify_with_budgets(
                    image(before, 212),
                    &mut budget(),
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .after_bytes(),
                after
            );
        }
    }
}
