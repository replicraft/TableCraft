//! The coordinator's original reading is independent of correlated staged facts.
use super::{
    api::{requests, same},
    checked, model, original,
};
use crate::EditError;
use crate::table::mutation::insert::{
    AuditRequest, audit, compose,
    omitted::{self, Original, clock, stage},
};
use model::Reading;
use tablecraft_expressions::functions::dates::clock::ClockReading;

#[test]
fn clock_audit_rejects_correlated_alternate_clock_rows_plan_and_candidate() {
    let case = model::source(6, false, "Date()", "Now()");
    let rows = model::added(&case, Reading::LeapNoon);
    let requests = requests(&case, &rows, true);
    let original = Original::Ordinary(&requests);
    let image = original::capture(&case.before);
    let checked_source =
        omitted::preview::qualify(&image, "Items", original, &mut original::budget(), &|| {
            false
        })
        .unwrap();
    let input = ClockReading::new(Reading::LeapNoon.input()).unwrap();
    let bound = clock::bind(
        &image,
        &checked_source,
        original,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let prepared = bound
        .resolve(Some(input), &mut original::budget(), &|| false)
        .unwrap();
    let plan = stage::consume(
        &image,
        &checked_source,
        prepared,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let after = compose::compose(&image, &plan, &mut original::budget(), &|| false).unwrap();
    same(&after, &model::expected(&case, &rows));
    audit::prove_with_clock(
        &image,
        "Items",
        AuditRequest::Omitted(original),
        &plan,
        &after,
        Some(input),
        &mut original::budget(),
        &|| false,
    )
    .unwrap();

    let alternate = ClockReading::new(Reading::EpochSix.input()).unwrap();
    let bound = clock::bind(
        &image,
        &checked_source,
        original,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let prepared = bound
        .resolve(Some(alternate), &mut original::budget(), &|| false)
        .unwrap();
    let plan = stage::consume(
        &image,
        &checked_source,
        prepared,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let after = compose::compose(&image, &plan, &mut original::budget(), &|| false).unwrap();
    same(
        &after,
        &model::expected(&case, &model::added(&case, Reading::EpochSix)),
    );
    checked(&after, 4);
    assert_eq!(
        audit::prove_with_clock(
            &image,
            "Items",
            AuditRequest::Omitted(original),
            &plan,
            &after,
            Some(input),
            &mut original::budget(),
            &|| false
        )
        .err(),
        Some(EditError::Preservation)
    );
}

#[test]
fn clock_audit_rejects_poisoned_materialized_value_under_the_original_reading() {
    let case = model::source(5, false, "Date()", "Now()");
    let rows = model::added(&case, Reading::LeapNoon);
    let requests = requests(&case, &rows, true);
    let original = Original::Ordinary(&requests);
    let image = original::capture(&case.before);
    let checked_source =
        omitted::preview::qualify(&image, "Items", original, &mut original::budget(), &|| {
            false
        })
        .unwrap();
    let input = ClockReading::new(Reading::LeapNoon.input()).unwrap();
    let bound = clock::bind(
        &image,
        &checked_source,
        original,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let prepared = bound
        .resolve(Some(input), &mut original::budget(), &|| false)
        .unwrap();
    let healthy = stage::consume(
        &image,
        &checked_source,
        prepared,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let after = compose::compose(&image, &healthy, &mut original::budget(), &|| false).unwrap();
    same(&after, &model::expected(&case, &rows));
    audit::prove_with_clock(
        &image,
        "Items",
        AuditRequest::Omitted(original),
        &healthy,
        &after,
        Some(input),
        &mut original::budget(),
        &|| false,
    )
    .unwrap();

    let bound = clock::bind(
        &image,
        &checked_source,
        original,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let mut prepared = bound
        .resolve(Some(input), &mut original::budget(), &|| false)
        .unwrap();
    let mut alternate = rows.clone();
    alternate[0].stamped = Some(0x3fd0000000000000);
    prepared.rows[0].values[2] = alternate[0].values().remove(2);
    let plan = stage::consume(
        &image,
        &checked_source,
        prepared,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let after = compose::compose(&image, &plan, &mut original::budget(), &|| false).unwrap();
    same(&after, &model::expected(&case, &alternate));
    checked(&after, 4);
    assert_eq!(
        audit::prove_with_clock(
            &image,
            "Items",
            AuditRequest::Omitted(original),
            &plan,
            &after,
            Some(input),
            &mut original::budget(),
            &|| false
        )
        .err(),
        Some(EditError::Preservation)
    );
}
