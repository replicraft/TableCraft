//! Opt-in requalification of existing hash-pinned independently observed images.
use super::{
    NativeFittedIntentGroup, NativeFittedUpdates, NativeUpdateProfile, SlotScalarUpdate, Value,
    budget, check_requalification, image,
};
use sha2::{Digest, Sha256};
use tablecraft_testkit::fixtures::access::{
    genuine_eof_allocation as eof, genuine_owned_growth as owned,
};

fn verify_pin(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn group(old: &str, replacement: &str) -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new(
            "Marker",
            NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
                0,
                Value::Text(old.to_owned()),
                Value::Text(replacement.to_owned()),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

#[test]
#[ignore = "requires existing hash-pinned genuine owned-growth image directories"]
fn owned_growth_requalifies_exact_original_profile_six_images() {
    for case in owned::load_all(verify_pin) {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_owned_growth_updates(
                "Sentinel",
                group(owned::OLD, owned::GROWN),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        check_requalification(
            plan,
            &case.before,
            &case.after,
            1,
            NativeUpdateProfile::Ace16OrdinaryOwnedGrowth,
        );
    }
}

#[test]
#[ignore = "requires existing hash-pinned genuine EOF-allocation image directory"]
fn eof_allocation_requalifies_complete_grown_images_and_original_profile_seven() {
    for case in eof::load_all(verify_pin) {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_eof_updates(
                "Sentinel",
                group(eof::OLD, eof::GROWN),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert!(case.after.len() > case.before.len());
        check_requalification(
            plan,
            &case.before,
            &case.after,
            1,
            NativeUpdateProfile::Ace16OrdinaryEofAllocation,
        );
    }
}
