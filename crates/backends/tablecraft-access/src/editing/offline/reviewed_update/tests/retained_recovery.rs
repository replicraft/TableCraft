//! Exact retained markers retain V3 ownership through filesystem session recovery.
use super::budget;
use crate::editing::offline::test_support::retained_update::assert_recovery;
use std::io::Cursor;
use tablecraft_ace::{
    FixedScalarUpdate, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage,
    NativeUpdateProfile,
};
use tablecraft_core::values::{Guid, Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    exact_values, fields, typed_fields,
};

fn image(before: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(2322),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn wide_value_marker_recovers_pending_committed_and_settled_v3_sessions() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let empty = exact_values::append_table(&seed, "ExactValues");
    let token = [0x42; 16];
    let replacement = [0x24; 16];
    let before = exact_values::append(&empty, definition, Some(token), Some(0));
    let after = exact_values::update_guid(&before, definition, 0, Some(replacement));
    assert_recovery(
        &before,
        &after,
        NativeUpdateProfile::Ace16RetainedWideValueRows,
        b"TCNI0022",
        || {
            let group = NativeFittedIntentGroup::new_retained_wide_values(
                "Token",
                NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                    0,
                    Value::Guid(Guid::from_bytes(token)),
                    Value::Guid(Guid::from_bytes(replacement)),
                )]),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            image(&before)
                .plan_recoverable_retained_wide_value_row_updates(
                    "ExactValues",
                    vec![group],
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
        },
    );
}

#[test]
fn missing_boolean_marker_recovers_pending_committed_and_settled_v3_sessions() {
    let source = fields::source(fields::Case::FirstNumber);
    let added = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Enabled",
        typed_fields::Kind::Boolean,
    );
    let before = typed_fields::boolean_historical(&added, source.definition, 0, true);
    let after = typed_fields::boolean_materialized_true(&before, source.definition, 0);
    assert_recovery(
        &before,
        &after,
        NativeUpdateProfile::Ace16RetainedBooleanMaterialization,
        b"TCNI0023",
        || {
            let group = NativeFittedIntentGroup::new_retained(
                "Enabled",
                NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                    0,
                    Value::Boolean(false),
                    Value::Boolean(true),
                )]),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            image(&before)
                .plan_recoverable_retained_boolean_materialization(
                    "Table1",
                    vec![group],
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
        },
    );
}
