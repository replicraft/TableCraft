//! Typed requests bind the same private row/index certificate as the old facade.
use super::{PAGE, ROOT, budget, capture, model};
use crate::table::mutation::split::{finish, fixed::FixedOperation, preflight, stage_checked};
use crate::{EditError, FixedScalarUpdate, Int32Update, WriteBlocker};
use tablecraft_core::values::Value;

#[test]
fn typed_original_requests_reject_unrequested_patch_values() {
    let bytes = model::source(6, false, true);
    let requests = [FixedScalarUpdate::new(
        0,
        Value::Int32(1),
        Value::Int32(i32::MAX),
    )];
    let operation = FixedOperation {
        table: "Items",
        column: "Score",
        requests: &requests,
    };
    let image = capture(&bytes);
    let structural = preflight(&image, &operation, &mut budget(), &|| false).unwrap();
    let mut staged =
        stage_checked::<4>(&image, &operation, structural, &mut budget(), &|| false).unwrap();
    let healthy = finish::bound(capture(&bytes), &operation, &staged, &mut budget(), &|| {
        false
    })
    .unwrap();
    model::assert_checked(healthy.candidate_bytes());
    // The staged field bytes cannot override the immutable typed request.
    staged.patches[0].after = (i32::MAX - 1).to_le_bytes();
    assert_eq!(
        finish::bound(image, &operation, &staged, &mut budget(), &|| false).err(),
        Some(EditError::Preservation)
    );
}

#[test]
fn duplicate_requests_precede_corrupt_indexes_and_exact_carrier_encoding() {
    let mut bytes = model::source(6, false, true);
    // Header capture remains possible; complete selected-table index checking fails.
    bytes[ROOT * PAGE] = 99;
    let old = [Int32Update::new(0, 1, 2), Int32Update::new(0, 1, 3)];
    assert_eq!(
        capture(&bytes)
            .preview_ace16_split_int32_updates("Items", "Score", &old, &mut budget(), &|| false)
            .err(),
        Some(EditError::Conflict)
    );
    let typed = [
        FixedScalarUpdate::new(0, Value::Int64(1), Value::Int64(2)),
        FixedScalarUpdate::new(0, Value::Int64(1), Value::Int64(3)),
    ];
    assert_eq!(
        capture(&bytes)
            .preview_ace16_split_fixed_updates("Items", "Score", &typed, &mut budget(), &|| false)
            .err(),
        Some(EditError::Conflict)
    );
    let healthy = model::source(6, false, true);
    assert_eq!(
        capture(&healthy)
            .preview_ace16_split_fixed_updates(
                "Items",
                "Score",
                &typed[..1],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::Column))
    );
}
