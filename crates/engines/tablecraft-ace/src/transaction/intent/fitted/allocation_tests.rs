//! Independent data-only TCNI7 wire; construction supplies no native permission.
use super::{NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates};
use crate::{
    FixedScalarUpdate, InspectError, NativeIntentError, NativeUpdateIntent, NativeUpdateProfile,
    SlotScalarUpdate,
};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

fn original() -> NativeFittedEditIntent {
    let groups = vec![
        NativeFittedIntentGroup::new(
            "Marker",
            NativeFittedUpdates::Variable(vec![
                SlotScalarUpdate::new(1, Value::Null, Value::Text(String::new())),
                SlotScalarUpdate::new(0, Value::Text("old".into()), Value::Text("雪\0".into())),
            ]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
        NativeFittedIntentGroup::new(
            "Empty",
            NativeFittedUpdates::Fixed(Vec::<FixedScalarUpdate>::new()),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ];
    NativeFittedEditIntent::new("Sentinel", groups, b"original", &mut budget(), &|| false).unwrap()
}

fn carrier(output: &mut Vec<u8>, tag: u8, bytes: &[u8]) {
    output.extend_from_slice(&[tag, 0, 0, 0]);
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(bytes);
}

fn literal(version: u8) -> Vec<u8> {
    let mut cells = Vec::new();
    for (ordinal, expected_tag, expected, replacement) in [
        (1_u64, 0_u8, &b""[..], &b""[..]),
        (0, 6, &b"old"[..], &b"\xe9\x9b\xaa\0"[..]),
    ] {
        let mut cell = Vec::new();
        cell.extend_from_slice(&ordinal.to_le_bytes());
        carrier(&mut cell, expected_tag, expected);
        carrier(&mut cell, 6, replacement);
        cells.extend_from_slice(&((4 + cell.len()) as u32).to_le_bytes());
        cells.extend_from_slice(&cell);
    }
    let mut body = Vec::from(*b"Sentinel");
    for number in [(20 + 6 + cells.len()) as u32, 6, 2, 1, 0] {
        body.extend_from_slice(&number.to_le_bytes());
    }
    body.extend_from_slice(b"Marker");
    body.extend_from_slice(&cells);
    for number in [25_u32, 5, 0, 0, 0] {
        body.extend_from_slice(&number.to_le_bytes());
    }
    body.extend_from_slice(b"Empty");
    let mut output = Vec::from(match version {
        5 => *b"TCNI0005",
        6 => *b"TCNI0006",
        7 => *b"TCNI0007",
        _ => unreachable!(),
    });
    output.extend_from_slice(&((72 + body.len()) as u32).to_le_bytes());
    output.extend_from_slice(&[version, 0, 0, 0]);
    output.extend_from_slice(&8_u64.to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update(8_u64.to_le_bytes());
    hash.update(b"original");
    output.extend_from_slice(&hash.finalize());
    for number in [8_u32, 2, 2, 0] {
        output.extend_from_slice(&number.to_le_bytes());
    }
    output.extend_from_slice(&body);
    output
}

#[test]
fn allocation_literal_keeps_exact_originals_and_old_versions_five_six() {
    for version in [5, 6, 7] {
        let intent = match version {
            5 => NativeUpdateIntent::from_fitted(original()),
            6 => NativeUpdateIntent::from_owned_growth(original()),
            7 => NativeUpdateIntent::from_eof_allocation(original()),
            _ => unreachable!(),
        };
        let expected = literal(version);
        assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), expected);
        let decoded = NativeUpdateIntent::decode(&expected, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), expected);
        assert_eq!(decoded.table(), "Sentinel");
        assert_eq!(decoded.before_length(), 8);
        decoded
            .verify_before(b"original", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            decoded.verify_before(b"originaX", &mut budget(), &|| false),
            Err(NativeIntentError::BeforeMismatch)
        );
        if version == 7 {
            assert_eq!(
                decoded.profile(),
                NativeUpdateProfile::Ace16OrdinaryEofAllocation
            );
            assert!(decoded.int32().is_none() && decoded.fixed().is_none());
            assert!(decoded.fitted().is_none() && decoded.owned_growth().is_none());
            let groups = decoded.eof_allocation().unwrap().groups();
            assert_eq!(groups.len(), 2);
            assert_eq!(groups[0].column(), "Marker");
            let NativeFittedUpdates::Variable(updates) = groups[0].updates() else {
                panic!("actual variable original body")
            };
            assert_eq!(updates[0].row(), 1);
            assert_eq!(updates[0].expected(), &Value::Null);
            assert_eq!(updates[0].value(), &Value::Text(String::new()));
            assert_eq!(updates[1].row(), 0);
            assert_eq!(updates[1].value(), &Value::Text("雪\0".into()));
            let NativeFittedUpdates::Fixed(updates) = groups[1].updates() else {
                panic!("retained fixed empty group")
            };
            assert!(updates.is_empty());
        }
    }
}

#[test]
fn allocation_version_profile_pairing_and_full_body_exhaustion_are_strict() {
    for version in [5, 6, 7] {
        for mismatch in [5, 6, 7] {
            if version == mismatch {
                continue;
            }
            let mut bad = literal(version);
            bad[12] = mismatch;
            assert!(matches!(
                NativeUpdateIntent::decode(&bad, &mut budget(), &|| false),
                Err(NativeIntentError::UnsupportedProfile)
            ));
        }
    }
    let good = literal(7);
    assert!(matches!(
        NativeFittedEditIntent::decode(&good, &mut budget(), &|| false),
        Err(NativeIntentError::UnsupportedVersion)
    ));
    for end in 0..good.len() {
        assert!(NativeUpdateIntent::decode(&good[..end], &mut budget(), &|| false).is_err());
    }
    for at in [14, 68, 93, 96, 118, 119] {
        let mut bad = good.clone();
        bad[at] = 255;
        assert!(NativeUpdateIntent::decode(&bad, &mut budget(), &|| false).is_err());
    }
    let mut bad_utf8 = good.clone();
    // Locate the unique literal UTF8 payload without using production
    // header, tag or offset constants.
    let at = bad_utf8
        .windows(4)
        .position(|bytes| bytes == b"\xe9\x9b\xaa\0")
        .unwrap();
    bad_utf8[at] = 255;
    assert!(NativeUpdateIntent::decode(&bad_utf8, &mut budget(), &|| false).is_err());
    for at in [56, 60, 64] {
        let mut excessive = good.clone();
        excessive[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(NativeUpdateIntent::decode(&excessive, &mut budget(), &|| false).is_err());
    }
    let mut trailing = good;
    trailing.push(0);
    assert!(NativeUpdateIntent::decode(&trailing, &mut budget(), &|| false).is_err());
}

#[test]
fn allocation_encode_decode_exact_pools_and_cancellation_remain_data_only() {
    let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let literal = literal(7);
    for encoding in [false, true] {
        let original = NativeUpdateIntent::from_eof_allocation(original());
        let run = |budget: &mut ResourceBudget, cancel: &dyn Fn() -> bool| {
            if encoding {
                original.encode(budget, cancel).map(|_| ())
            } else {
                NativeUpdateIntent::decode(&literal, budget, cancel).map(|_| ())
            }
        };
        let mut measured = budget();
        let initial = pools.map(|pool| measured.remaining(pool));
        run(&mut measured, &|| false).unwrap();
        let costs: [u64; 3] =
            std::array::from_fn(|index| initial[index] - measured.remaining(pools[index]));
        assert_eq!(costs[1], 0);
        let mut exact = ResourceBudget::new(costs[0], 0, costs[2]);
        run(&mut exact, &|| false).unwrap();
        assert_eq!(
            (
                exact.remaining(Resource::Bytes),
                exact.remaining(Resource::Steps)
            ),
            (0, 0)
        );
        for index in [0, 2] {
            let mut below = costs;
            below[index] -= 1;
            assert!(
                matches!(run(&mut ResourceBudget::new(below[0], 0, below[2]), &|| false),
                Err(NativeIntentError::Inspect(InspectError::Limit(error))) if error.resource == pools[index])
            );
        }
        let polls = Cell::new(0_usize);
        run(&mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        for stop in [1, polls.get().div_ceil(2), polls.get()] {
            let seen = Cell::new(0_usize);
            assert!(matches!(
                run(&mut budget(), &|| {
                    seen.set(seen.get() + 1);
                    seen.get() >= stop
                }),
                Err(NativeIntentError::Inspect(InspectError::Cancelled))
            ));
        }
    }
}
