//! Native typed selection uses the same complete sealed DELETE candidate.
use super::*;
use crate::NativeCollationResolver;
use std::{cell::RefCell, io::Cursor};
use tablecraft_ace::{NativeSortOrder, read_table};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::{canonical_creation, canonical_native17_creation};

const SCOPE: CatalogScope = CatalogScope::new(259);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn typed_delete_context_selects_original_rows_and_legacy_api_still_refuses_declarations() {
    let before = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Boundary).bytes,
    );
    let parsed = parse_query(
        "PARAMETERS minimum LONG; DELETE FROM Items WHERE Id>=minimum",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        local_delete_table(parsed.query(), false),
        Err(QueryError::Unsupported)
    ));
    let image =
        NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false).unwrap();
    let comparison_pool = RefCell::new(budget());
    let resolver = NativeCollationResolver::new_deferred_default(
        NativeSortOrder::from_native_bytes([0, 0, 0, 0]),
        &comparison_pool,
        &|| false,
    )
    .unwrap();
    let parameters = [("minimum", Value::Int32(63))];
    let plan = plan_recoverable_delete_with_options(
        image,
        parsed.query(),
        &mut budget(),
        &mut budget(),
        NativeMutationOptions {
            parameters: &parameters,
            collation: resolver.default_collation(),
            resolver: &resolver,
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(plan.matched_rows(), 2);
    assert_eq!(plan.backend().deleted_rows(), 2);
    assert_eq!(plan.backend().before_bytes(), before);
    let rows = read_table(
        &mut Cursor::new(plan.backend().after_bytes()),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows.rows().len(), 62);
    assert_eq!(rows.rows()[0], [Value::Int32(1), Value::Int32(10)]);
    assert_eq!(rows.rows()[61], [Value::Int32(62), Value::Int32(620)]);
}
