//! The declared compressed form cannot displace an otherwise fitting raw form.
use super::{budget, capture, framed, groups, requests, same_image};
use crate::{
    EditError, SlotScalarUpdate, WriteBlocker,
    table::mutation::mixed_fitted_tests::{capacity, model::Family},
};
use tablecraft_core::values::Value;

#[test]
fn raw_one_character_fits_zero_slack_in_one_joint_plan() {
    for family in [Family::Legacy, Family::Modern] {
        for descending in [false, true] {
            let case = framed::framed(capacity::case(family, descending));
            let (fixed, variable) = requests(&case);
            let mut groups = groups(&fixed, &variable);

            for _ in 0..2 {
                let result = capture(&case.before)
                    .preview_ace16_fitted_updates("Items", &groups, &mut budget(), &|| false)
                    .unwrap();
                same_image(result.before_bytes(), &case.before);
                same_image(result.candidate_bytes(), &case.expected);
                assert_eq!(result.changed_rows(), 2);
                groups.reverse();
            }
        }
    }
}

#[test]
fn neither_legal_form_fits_the_original_capacity() {
    let case = framed::framed(capacity::case(Family::Legacy, false));
    let (fixed, mut variable) = requests(&case);
    variable[0] = SlotScalarUpdate::new(0, Value::Text("a".into()), Value::Text("aaa".into()));
    let result = capture(&case.before).preview_ace16_fitted_updates(
        "Items",
        &groups(&fixed, &variable),
        &mut budget(),
        &|| false,
    );
    assert!(matches!(
        result,
        Err(EditError::Unsupported(WriteBlocker::RowLayout)),
    ));
}
