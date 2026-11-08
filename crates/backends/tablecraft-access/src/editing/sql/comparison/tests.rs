//! Actual native field declarations survive the adapter's scalar projection.
use super::{align, schema};
use crate::{
    NativeCollationResolver,
    editing::{
        NativeEditError,
        sql::{
            NativeMutationOptions, RecoverableError, plan_recoverable_batch_update_with_options,
        },
    },
};
use std::{cell::RefCell, io::Cursor};
use tablecraft_ace::{InspectError, NativeImage, NativeSortOrder};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::EvalError;
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_ordinary_fitted::{self as fitted, Recipe};

const SCOPE: CatalogScope = CatalogScope::new(245);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn actual_text_field_policy_is_used_even_when_the_explicit_caller_default_refuses() {
    for header in [5, 6] {
        let case = fitted::source(header, Recipe::ExactNoops);
        let before = case.before;
        let comparison_pool = RefCell::new(budget());
        // The supplied default is deliberately unavailable. The native header
        // stays unchanged; this tests the explicit adapter-provider boundary.
        let resolver = NativeCollationResolver::new_deferred_default(
            NativeSortOrder::from_native_bytes([0, 0, 0, 0]),
            &comparison_pool,
            &|| false,
        )
        .unwrap();
        let parsed = parse_query(
            "UPDATE Items SET Id=77 WHERE TextValue='ALPHA'",
            ParseLimits::default(),
        )
        .unwrap();
        let plan = plan_recoverable_batch_update_with_options(
            image(&before),
            parsed.query(),
            &mut budget(),
            &mut budget(),
            NativeMutationOptions {
                parameters: &[],
                collation: resolver.default_collation(),
                resolver: &resolver,
            },
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.matched_rows(), 1);
        assert_eq!(plan.backend().changed_rows(), 1);
        let mut expected = before.clone();
        let start = case.frames[0].absolute + 2;
        assert_eq!(&expected[start..start + 4], &1_i32.to_le_bytes());
        expected[start..start + 4].copy_from_slice(&77_i32.to_le_bytes());
        assert_eq!(plan.backend().before_bytes(), before);
        assert_eq!(plan.backend().after_bytes(), expected);

        let literal = parse_query(
            "UPDATE Items SET Id=77 WHERE 'a'='A'",
            ParseLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            plan_recoverable_batch_update_with_options(
                image(&before),
                literal.query(),
                &mut budget(),
                &mut budget(),
                NativeMutationOptions {
                    parameters: &[],
                    collation: resolver.default_collation(),
                    resolver: &resolver
                },
                &|| false,
            ),
            Err(RecoverableError::Plan(UpdateError::Query(
                QueryError::Evaluation(EvalError::CollationRequired)
            )))
        ));
    }
}

#[test]
fn projected_scalar_policy_alignment_uses_names_and_types_instead_of_native_positions() {
    let case = fitted::source(5, Recipe::ExactNoops);
    let image = image(&case.before);
    let schema = schema(&image, "Items", &mut budget(), &|| false).unwrap();
    // A real projection can omit unsupported siblings and reorder its retained
    // scalars. Query policy metadata must follow the actual snapshot columns.
    let columns = [
        schema.columns()[2].column().unwrap(),
        schema.columns()[1].column().unwrap(),
        schema.columns()[0].column().unwrap(),
    ];
    let policies = align(&schema, &columns, &mut budget(), &|| false).unwrap();
    assert_eq!(policies.len(), columns.len());
    assert!(policies[0].is_none());
    assert_eq!(
        policies[1],
        Some(crate::collation::identity(
            schema.columns()[1].text_sort_order().unwrap()
        ))
    );
    assert!(policies[2].is_none());

    for column in [
        Column::new("Missing".to_owned(), ScalarType::Int32, true).unwrap(),
        Column::new("TextValue".to_owned(), ScalarType::Int32, true).unwrap(),
    ] {
        assert!(matches!(
            align(&schema, &[column], &mut budget(), &|| false),
            Err(NativeEditError::Inspect(InspectError::Malformed))
        ));
    }
    assert!(matches!(
        align(&schema, &columns, &mut budget(), &|| true),
        Err(NativeEditError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        align(&schema, &columns, &mut ResourceBudget::new(0, 100_000, 100_000_000), &|| false),
        Err(NativeEditError::Inspect(InspectError::Limit(error))) if error.resource == Resource::Bytes
    ));
}

#[test]
fn native_metadata_refusal_cannot_consume_or_refill_the_independent_query_allowance() {
    let case = fitted::source(5, Recipe::ExactNoops);
    let parsed = parse_query(
        "UPDATE Items SET Id=77 WHERE TextValue='ALPHA'",
        ParseLimits::default(),
    )
    .unwrap();
    let comparison_pool = RefCell::new(budget());
    let resolver = NativeCollationResolver::new_deferred_default(
        NativeSortOrder::from_native_bytes([9, 4, 0, 1]),
        &comparison_pool,
        &|| false,
    )
    .unwrap();
    for cancelled in [false, true] {
        let mut query = budget();
        let result = plan_recoverable_batch_update_with_options(
            image(&case.before),
            parsed.query(),
            &mut ResourceBudget::new(0, 0, 0),
            &mut query,
            NativeMutationOptions {
                parameters: &[],
                collation: resolver.default_collation(),
                resolver: &resolver,
            },
            &|| cancelled,
        );
        if cancelled {
            assert!(matches!(
                result,
                Err(RecoverableError::Editor(NativeEditError::Inspect(
                    InspectError::Cancelled
                )))
            ));
        } else {
            assert!(
                matches!(result, Err(RecoverableError::Editor(NativeEditError::Inspect(InspectError::Limit(error)))) if error.resource == Resource::Steps)
            );
        }
        assert_eq!(query, budget());
    }
}

#[test]
fn explicit_native_parameter_context_preserves_legacy_declaration_refusal() {
    let before = tablecraft_testkit::fixtures::access::mutation_fixture::database();
    let expected = tablecraft_testkit::fixtures::access::mutation_fixture::with_rows(&[
        (1, 10),
        (2, 25),
        (3, 30),
    ]);
    let parsed = parse_query(
        "PARAMETERS replacement LONG, selected LONG; UPDATE Items SET Score=replacement WHERE Id=selected",
        ParseLimits::default(),
    ).unwrap();
    assert!(matches!(
        crate::editing::sql::local_batch_update_table(parsed.query(), false),
        Err(QueryError::Unsupported)
    ));
    assert!(matches!(
        crate::editing::sql::plan_recoverable_batch_update(
            image(&before),
            parsed.query(),
            &mut budget(),
            None,
            &|| false,
        ),
        Err(RecoverableError::Shape(QueryError::Unsupported))
    ));
    let comparison_pool = RefCell::new(budget());
    let resolver = NativeCollationResolver::new_deferred_default(
        NativeSortOrder::from_native_bytes([9, 4, 0, 1]),
        &comparison_pool,
        &|| false,
    )
    .unwrap();
    let parameters = [
        ("replacement", tablecraft_core::values::Value::Int32(25)),
        ("selected", tablecraft_core::values::Value::Int32(2)),
    ];
    let plan = plan_recoverable_batch_update_with_options(
        image(&before),
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
    assert_eq!(plan.matched_rows(), 1);
    assert_eq!(plan.backend().changed_rows(), 1);
    assert_eq!(plan.backend().before_bytes(), before);
    assert_eq!(plan.backend().after_bytes(), expected);
}
