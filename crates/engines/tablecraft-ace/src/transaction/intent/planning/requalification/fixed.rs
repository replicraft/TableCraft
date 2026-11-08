//! Remaining closed fixed dialects use independent raw payload/event goldens.
use super::{budget, check_requalification, image};
use crate::{FixedScalarUpdate, NativeFixedIntentGroup, NativeUpdateProfile};
use tablecraft_core::values::{Guid, Single, Value};
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_before_change as events, canonical_fixed_creation as fixed,
    canonical_native17_creation as native17,
};

// Existing raw fixed SOURCE packs its first 63-byte record at DATA page 8's
// end. Field payload offsets below include the two-byte field-count prefix.
const FIRST_RECORD: usize = 9 * 4096 - 63;

#[test]
fn before_change_fixed_requalifies_complete_event_and_user_effects_in_profile_three() {
    for header in [5, 6] {
        let case = events::source(header, events::Recipe::JointSharedMask);
        assert!(case.refusal.is_none());
        let groups = case
            .groups
            .iter()
            .map(|group| {
                let (_, name, _, _) = events::FIELDS
                    .iter()
                    .find(|(number, _, _, _)| *number == group.column)
                    .unwrap();
                let updates = group
                    .updates
                    .iter()
                    .map(|update| {
                        FixedScalarUpdate::new(
                            update.ordinal,
                            update.expected.value(),
                            update.replacement.value(),
                        )
                    })
                    .collect();
                NativeFixedIntentGroup::new(name, updates, &mut budget(), &|| false).unwrap()
            })
            .collect();
        let plan = image(&case.before)
            .plan_recoverable_fixed_updates("Items", groups, &mut budget(), &|| false)
            .unwrap();
        check_requalification(
            plan,
            &case.before,
            case.expected.as_ref().unwrap(),
            case.changed_rows,
            NativeUpdateProfile::Ace16BeforeChangeFixed,
        );
    }
}

#[test]
fn ieee_scalar_and_guid_dialects_requalify_complete_independent_payload_overlays() {
    for header in [5, 6] {
        let source = fixed::source(fixed::Case::Endpoints);
        let mut before = native17::source(source.bytes);
        before[20] = header;
        let cases = [
            (
                NativeUpdateProfile::Ace16OrdinaryIeeeFixed,
                "SingleValue",
                6,
                25,
                Value::Single(Single::from_bits(0x8000_0000)),
                vec![0, 0, 0, 128],
            ),
            (
                NativeUpdateProfile::Ace16OrdinaryScalarFixed,
                "ByteValue",
                1,
                2,
                Value::Byte(7),
                vec![7],
            ),
            (
                NativeUpdateProfile::Ace16OrdinaryGuidFixed,
                "GuidValue",
                9,
                45,
                Value::Guid(Guid::from_bytes([
                    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
                ])),
                vec![3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15],
            ),
        ];
        for (profile, column, index, offset, replacement, raw) in cases {
            let updates = vec![FixedScalarUpdate::new(
                0,
                source.rows[0][index].clone(),
                replacement,
            )];
            let group = match profile {
                NativeUpdateProfile::Ace16OrdinaryIeeeFixed => {
                    NativeFixedIntentGroup::new_ordinary_ieee(
                        column,
                        updates,
                        &mut budget(),
                        &|| false,
                    )
                }
                NativeUpdateProfile::Ace16OrdinaryScalarFixed => {
                    NativeFixedIntentGroup::new_ordinary_scalar(
                        column,
                        updates,
                        &mut budget(),
                        &|| false,
                    )
                }
                NativeUpdateProfile::Ace16OrdinaryGuidFixed => {
                    NativeFixedIntentGroup::new_ordinary_guid(
                        column,
                        updates,
                        &mut budget(),
                        &|| false,
                    )
                }
                _ => unreachable!(),
            }
            .unwrap();
            let mut expected = before.clone();
            let start = FIRST_RECORD + offset;
            expected[start..start + raw.len()].copy_from_slice(&raw);
            let plan = image(&before)
                .plan_recoverable_ordinary_fixed_updates(
                    "Items",
                    vec![group],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            check_requalification(plan, &before, &expected, 1, profile);
        }
    }
}
