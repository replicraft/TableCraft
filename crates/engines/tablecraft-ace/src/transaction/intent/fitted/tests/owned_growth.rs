//! Profile6 metadata is data, with no conversion from a preview or final image.
use super::{budget, literal, original};
use crate::{
    InspectError, NativeIntentError, NativeUpdateBody, NativeUpdateIntent, NativeUpdateProfile,
};
use std::{array, cell::Cell};
use tablecraft_core::limits::{Resource, ResourceBudget};

fn owned_wire() -> Vec<u8> {
    // Reuse the independent existing587B record body. Only the two separately
    // declared closed operation identifiers differ; no production encoder runs.
    let mut bytes = literal();
    assert_eq!(bytes.len(), 587);
    bytes[..8].copy_from_slice(b"TCNI0006");
    bytes[12..14].copy_from_slice(&6_u16.to_le_bytes());
    bytes
}

#[test]
fn owned_growth_wire_retains_original_groups_but_never_changes_tcni5() {
    let fitting = original(&mut budget(), &|| false).unwrap();
    assert_eq!(fitting.encode(&mut budget(), &|| false).unwrap(), literal());
    let owned = NativeUpdateIntent::from_owned_growth(fitting);
    let bytes = owned_wire();
    assert_eq!(
        owned.profile(),
        NativeUpdateProfile::Ace16OrdinaryOwnedGrowth
    );
    assert!(matches!(owned.body(), NativeUpdateBody::OwnedGrowth(_)));
    assert!(owned.int32().is_none());
    assert!(owned.fixed().is_none());
    assert!(owned.fitted().is_none());
    let groups = owned.owned_growth().unwrap().groups();
    assert_eq!(groups.len(), 8);
    assert_eq!(groups[0].column(), "Text");
    assert_eq!(groups[3].column(), "Empty");
    assert_eq!(owned.encode(&mut budget(), &|| false).unwrap(), bytes);
    let decoded = NativeUpdateIntent::decode(&bytes, &mut budget(), &|| false).unwrap();
    assert_eq!(
        decoded.profile(),
        NativeUpdateProfile::Ace16OrdinaryOwnedGrowth
    );
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), bytes);
    decoded
        .verify_before(b"original", &mut budget(), &|| false)
        .unwrap();
    let legacy = NativeUpdateIntent::decode(&literal(), &mut budget(), &|| false).unwrap();
    assert_eq!(legacy.profile(), NativeUpdateProfile::Ace16OrdinaryFitted);
    assert!(legacy.owned_growth().is_none());
    assert_eq!(legacy.encode(&mut budget(), &|| false).unwrap(), literal());
    assert!(matches!(
        super::super::NativeFittedEditIntent::decode(&bytes, &mut budget(), &|| false),
        Err(NativeIntentError::UnsupportedVersion)
    ));
}

#[test]
fn owned_growth_version_profile_padding_and_body_refuse_before_decoded_allocation() {
    for mutation in 0..5 {
        let mut bytes = owned_wire();
        match mutation {
            0 => bytes[7] = b'8',
            1 => bytes[12] = 5,
            2 => bytes[14] = 1,
            3 => bytes.push(0),
            4 => bytes[7] = b'7',
            _ => unreachable!(),
        }
        let mut zero_bytes = ResourceBudget::new(0, 0, 1_000_000_000);
        let error = NativeUpdateIntent::decode(&bytes, &mut zero_bytes, &|| false)
            .err()
            .unwrap();
        let expected = match mutation {
            0 => NativeIntentError::UnsupportedVersion,
            1 | 4 => NativeIntentError::UnsupportedProfile,
            _ => NativeIntentError::InvalidRecord,
        };
        assert_eq!(error, expected);
        assert_eq!(zero_bytes.remaining(Resource::Bytes), 0);
    }
}

#[test]
fn owned_growth_decode_and_encode_pay_exact_pools_and_cancel_without_rows() {
    let bytes = owned_wire();
    let intent = NativeUpdateIntent::decode(&bytes, &mut budget(), &|| false).unwrap();
    for encode in [false, true] {
        let run = |work: &mut ResourceBudget, cancel: &dyn Fn() -> bool| {
            if encode {
                intent.encode(work, cancel).map(|_| ())
            } else {
                NativeUpdateIntent::decode(&bytes, work, cancel).map(|_| ())
            }
        };
        let mut measured = budget();
        let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
        let initial = pools.map(|pool| measured.remaining(pool));
        run(&mut measured, &|| false).unwrap();
        let costs: [u64; 3] =
            array::from_fn(|index| initial[index] - measured.remaining(pools[index]));
        assert_eq!(costs[1], 0);
        run(&mut ResourceBudget::new(costs[0], 0, costs[2]), &|| false).unwrap();
        for (index, pool) in [(0, Resource::Bytes), (2, Resource::Steps)] {
            let mut below = costs;
            assert!(below[index] > 0);
            below[index] -= 1;
            assert!(
                matches!(run(&mut ResourceBudget::new(below[0], 0, below[2]), &|| false), Err(NativeIntentError::Inspect(InspectError::Limit(error))) if error.resource == pool)
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
