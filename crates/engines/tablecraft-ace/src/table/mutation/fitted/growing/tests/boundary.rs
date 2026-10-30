//! A source-valid map boundary permits no-ops but grants no promotion fallback.
use super::{budget, capture, groups, same_image};
use crate::{
    EditError, FixedScalarUpdate, InspectError, SlotScalarUpdate,
    table::mutation::mixed_fitted_tests::growing_boundary,
};
use tablecraft_core::values::Value;

#[test]
fn represented_candidate_frontier_refuses_before_any_candidate() {
    let before = growing_boundary::source();
    let fixed = [FixedScalarUpdate::new(3, Value::Null, Value::Null)];
    let variable = [SlotScalarUpdate::new(3, Value::Null, Value::Null)];
    let unchanged = capture(&before)
        .preview_ace16_mixed_updates("Items", &groups(&fixed, &variable), &mut budget(), &|| {
            false
        })
        .unwrap();
    same_image(unchanged.candidate_bytes(), &before);
    assert_eq!(unchanged.changed_rows(), 0);

    let fixed = [FixedScalarUpdate::new(3, Value::Null, Value::Int32(40))];
    let variable = [SlotScalarUpdate::new(
        3,
        Value::Null,
        Value::Text("aa".into()),
    )];
    assert!(matches!(
        capture(&before).preview_ace16_mixed_updates(
            "Items",
            &groups(&fixed, &variable),
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Inspect(InspectError::Unsupported)),
    ));
}
