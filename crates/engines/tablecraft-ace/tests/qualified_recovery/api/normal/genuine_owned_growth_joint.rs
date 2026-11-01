//! Intended child of api::normal; literal joint sources precede producer calls.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_ace::{
    FixedScalarUpdate, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage,
    NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile, SlotScalarUpdate,
    qualify_native_update_recovery, read_table,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};

struct Case {
    before: Vec<u8>,
    after: Vec<u8>,
    id: Option<i32>,
    data: usize,
}

// Every file is source-bound before the caller can create the first target.
fn cases() -> Vec<Case> {
    let originals =
        PathBuf::from(std::env::var_os("TABLECRAFT_PROPERTY_LVAL_ORIGINALS_DIR").unwrap());
    let expected =
        PathBuf::from(std::env::var_os("TABLECRAFT_OWNED_GROWTH_JOINT_EXPECTED_DIR").unwrap());
    assert!(originals.is_absolute() && expected.is_absolute());
    let mut result = Vec::new();
    for (header, digest, data) in [
        (
            5_u8,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            110_usize,
        ),
        (
            6_u8,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            127_usize,
        ),
    ] {
        let before = fs::read(originals.join(format!("h{header}.accdb"))).unwrap();
        let actual = Sha256::digest(&before)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, digest);
        let base = data * 4096;
        for (recipe, id) in [("id-null", None), ("id-twenty-four", Some(24_i32))] {
            let after =
                fs::read(expected.join(format!("h{header}-marker-growth-{recipe}.accdb"))).unwrap();
            let original = &before[base + 4030..base + 4096];
            assert_eq!(&original[..6], &[3, 0, 23, 0, 0, 0]);
            let mut terminal = original[..6].to_vec();
            if let Some(id) = id {
                terminal[2..6].copy_from_slice(&id.to_le_bytes());
            }
            for _ in 0..19 {
                terminal.extend_from_slice(&[97, 0]);
            }
            terminal.extend_from_slice(&original[42..57]);
            terminal.extend_from_slice(&[
                59,
                0,
                44,
                0,
                6,
                0,
                2,
                0,
                if id.is_some() { 7 } else { 6 },
            ]);
            assert_eq!(terminal.len(), 68);
            let mut literal = before.clone();
            literal[base + 3962..base + 4030].copy_from_slice(&terminal);
            literal[base + 4030..base + 4096].fill(0);
            literal[base + 4030..base + 4034]
                .copy_from_slice(&(((data as u32) << 8) | 1).to_le_bytes());
            literal[base + 2..base + 4].copy_from_slice(&[104, 15]);
            literal[base + 12..base + 18].copy_from_slice(&[2, 0, 190, 79, 122, 143]);
            assert_eq!(after, literal);
            result.push(Case {
                before: before.clone(),
                after,
                id,
                data,
            });
        }
    }
    assert_eq!(result.len(), 4);
    result
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn value(id: Option<i32>) -> Value {
    id.map_or(Value::Null, Value::Int32)
}

fn groups(case: &Case, reverse: bool) -> Vec<NativeFittedIntentGroup> {
    let mut groups = vec![
        NativeFittedIntentGroup::new(
            "Marker",
            NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
                0,
                Value::Text("unchanged sentinel".into()),
                Value::Text("aaaaaaaaaaaaaaaaaaa".into()),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
        NativeFittedIntentGroup::new(
            "ID",
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                0,
                Value::Int32(23),
                value(case.id),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ];
    if reverse {
        groups.reverse();
    }
    groups
}

fn carrier(output: &mut Vec<u8>, value: &Value) {
    let (tag, bytes) = match value {
        Value::Null => (0, Vec::new()),
        Value::Int32(value) => (2, value.to_le_bytes().to_vec()),
        Value::Text(value) => (6, value.as_bytes().to_vec()),
        _ => unreachable!(),
    };
    output.extend_from_slice(&[tag, 0, 0, 0]);
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(&bytes);
}

fn wire(case: &Case, reverse: bool) -> Vec<u8> {
    let mut original = vec![
        (
            "Marker",
            1_u32,
            Value::Text("unchanged sentinel".into()),
            Value::Text("aaaaaaaaaaaaaaaaaaa".into()),
        ),
        ("ID", 0, Value::Int32(23), value(case.id)),
    ];
    if reverse {
        original.reverse();
    }
    let mut body = Vec::from(*b"Sentinel");
    for (column, family, old, new) in original {
        let mut cell = Vec::new();
        cell.extend_from_slice(&0_u64.to_le_bytes());
        carrier(&mut cell, &old);
        carrier(&mut cell, &new);
        for number in [
            (20 + column.len() + 4 + cell.len()) as u32,
            column.len() as u32,
            1,
            family,
            0,
        ] {
            body.extend_from_slice(&number.to_le_bytes());
        }
        body.extend_from_slice(column.as_bytes());
        body.extend_from_slice(&((cell.len() + 4) as u32).to_le_bytes());
        body.extend_from_slice(&cell);
    }
    let mut output = Vec::from(*b"TCNI0006");
    output.extend_from_slice(&((72 + body.len()) as u32).to_le_bytes());
    output.extend_from_slice(&[6, 0, 0, 0]);
    output.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    output.extend_from_slice(&digest.finalize());
    for number in [8_u32, 2, 2, 0] {
        output.extend_from_slice(&number.to_le_bytes());
    }
    output.extend_from_slice(&body);
    output
}

#[test]
#[ignore = "requires separately source-qualified joint growth images"]
fn joint_growth_fixed_mask_tracks_new_footer_in_both_original_group_orders() {
    let mut commits = 0;
    for case in cases() {
        for reverse in [false, true] {
            let plan = image(&case.before)
                .plan_recoverable_ordinary_owned_growth_updates(
                    "Sentinel",
                    groups(&case, reverse),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.changed_rows(), 1);
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryOwnedGrowth
            );
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                wire(&case, reverse)
            );
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                1
            );
            assert_eq!(target.bytes.get_ref(), &case.after);
            let terminal = case.data * 4096 + 3962;
            assert_eq!(target.bytes.get_ref()[terminal + 65], 2); // variable count, NOT old mask
            assert_eq!(
                target.bytes.get_ref()[terminal + 67],
                if case.id.is_some() { 7 } else { 6 }
            );
            if case.id.is_none() {
                assert_eq!(
                    &target.bytes.get_ref()[terminal + 2..terminal + 6],
                    &[23, 0, 0, 0]
                );
            }
            let snapshot = read_table(
                &mut Cursor::new(target.bytes.get_ref().as_slice()),
                scope(),
                "Sentinel",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                snapshot.rows(),
                [vec![
                    value(case.id),
                    Value::Text("aaaaaaaaaaaaaaaaaaa".into()),
                    Value::Binary(vec![199, 0, 255])
                ]]
            );
            commits += 1;
        }
    }
    assert_eq!(commits, 8);
}

#[test]
#[ignore = "requires separately source-qualified joint growth images"]
fn joint_growth_recovery_preserves_inactive_id_and_refuses_old_mask_position_forgery() {
    for case in cases() {
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            let committed = matches!(marker, Marker::Committed);
            let outcome = if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            };
            let mut target = Storage::new(case.after.clone());
            let mut journal = Storage::new(journals::journal(&case.before, &case.after, marker));
            let decoded =
                NativeUpdateIntent::decode(&wire(&case, false), &mut budget(), &|| false).unwrap();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified =
                qualify_native_update_recovery(checked, decoded, scope(), &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(
                qualified
                    .recover(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                outcome
            );
            assert_eq!(
                target.bytes.get_ref(),
                if committed { &case.after } else { &case.before }
            );
        }
        let mut forged = case.after.clone();
        forged[case.data * 4096 + 3962 + 65] ^= 1;
        let mut journal = Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
        let saved = journal.bytes.get_ref().clone();
        let decoded =
            NativeUpdateIntent::decode(&wire(&case, false), &mut budget(), &|| false).unwrap();
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_update_recovery(checked, decoded, scope(), &mut budget(), &|| false),
            Err(NativeRecoveryQualificationError::ImageMismatch)
        ));
        assert_eq!(journal.writes, 0);
        assert_eq!(journal.bytes.get_ref(), &saved);
    }
}
