//! Typed SQL values preserve exact complete or original-presence native intent.
use super::*;
use crate::NativeCollationResolver;
use std::{cell::RefCell, io::Cursor};
use tablecraft_ace::NativeSortOrder;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_creation_defaults as defaults;

const SCOPE: CatalogScope = CatalogScope::new(261);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn native_typed_values_preserve_omitted_default_and_explicit_null_whole_images() {
    for (sql, supplied_null) in [
        (
            "PARAMETERS added LONG; INSERT INTO Items (Id) VALUES (added)",
            false,
        ),
        (
            "PARAMETERS added LONG; INSERT INTO Items (Id,Score) VALUES (added,NULL)",
            true,
        ),
    ] {
        let source = defaults::source(defaults::Case::Seven);
        let (expected, _) = defaults::inserted(&source, supplied_null).unwrap();
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(local_insert_table(parsed.query(), false, &mut budget(), &|| false).is_err());
        let image = NativeImage::capture(
            &mut Cursor::new(&source.bytes),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let comparison = RefCell::new(budget());
        let resolver = NativeCollationResolver::new_deferred_default(
            NativeSortOrder::from_native_bytes([0, 0, 0, 0]),
            &comparison,
            &|| false,
        )
        .unwrap();
        let parameters = [("added", Value::Int32(if supplied_null { 25 } else { 24 }))];
        let plan = plan_recoverable_insert_with_options(
            image,
            parsed.query(),
            &mut budget(),
            &mut budget(),
            NativeMutationOptions {
                parameters: &parameters,
                collation: resolver.default_collation(),
                resolver: &resolver,
            },
            None,
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.before_bytes(), source.bytes);
        assert!(
            plan.after_bytes() == expected,
            "complete independent INSERT image differs"
        );
        assert_eq!(plan.inserted_rows(), 1);
        assert_eq!(
            matches!(plan, NativeJournalInsertPlan::Omitted(_)),
            !supplied_null
        );
    }
}
