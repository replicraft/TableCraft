//! Typed DROP journals recover exact schema effects without touching row bytes.
use crate::editing::offline::test_support::saved_field::{
    FieldPlan, assert_retained_schema_recovery,
};
use std::io::Cursor;
use tablecraft_ace::{NativeImage, RecoverableFieldDropPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    exact_values as raw, field_changes, fields,
};

const PAGE_BYTES: usize = 4096;
const CATALOG_PAGE: usize = 4;
const ROW_COUNT_OFFSET: usize = 12;
const SCOPE: CatalogScope = CatalogScope::new(1612);
const TOKEN: [u8; 16] = [
    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn review(before: &[u8], column: &str) -> RecoverableFieldDropPlan {
    NativeImage::capture_retained(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
        .plan_recoverable_retained_typed_tail_field_drop(
            "ExactValues",
            column,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

#[test]
fn typed_tail_drop_cold_and_settled_recovery_preserve_retired_payloads_and_schema_receipts() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / PAGE_BYTES) as u32;
    let count_at = CATALOG_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET;
    let catalog_root = usize::from(u16::from_le_bytes(
        seed[count_at..count_at + size_of::<u16>()]
            .try_into()
            .unwrap(),
    ));
    let empty = raw::append_table(&seed, "ExactValues");
    let mut before = raw::append(
        &empty,
        definition,
        Some(TOKEN),
        Some(90_071_992_547_409_931_234),
    );

    // The second source retains the first field's retired variable slot and
    // payload. Each action must freshly qualify that actual history.
    for column in ["Amount", "Token"] {
        let after = field_changes::drop_tail(&before, definition, catalog_root, column);
        for committed in [false, true] {
            for retired in [false, true] {
                let plan = review(&before, column);
                assert!(plan.intent().is_retained_typed());
                assert_eq!(plan.after_bytes().len(), after.len());
                assert_eq!(
                    plan.after_bytes()
                        .iter()
                        .zip(&after)
                        .position(|(left, right)| left != right),
                    None,
                    "first differing complete candidate byte",
                );
                assert_retained_schema_recovery(
                    SCOPE,
                    &before,
                    &after,
                    FieldPlan::Drop(plan),
                    committed,
                    retired,
                    &mut budget(),
                );
            }
        }
        before = after;
    }
}
