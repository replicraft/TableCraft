//! Healthy full images precede correlated cached-row/candidate forgeries.
use super::*;
use crate::table::mutation::insert::{
    AuditRequest, audit, compose,
    omitted::{self, InsertFieldValue, OmittedRowInsert, Original, stage},
};
use crate::{EditError, NativeImage};
use tablecraft_core::values::Value;

fn request(row: &Row, omitted: Option<usize>) -> OmittedRowInsert {
    let values = row
        .values(true)
        .into_iter()
        .enumerate()
        .map(|(position, value)| {
            if omitted == Some(position) {
                InsertFieldValue::Omitted
            } else {
                InsertFieldValue::Supplied(value)
            }
        })
        .collect();
    OmittedRowInsert::new(values)
}

fn difference(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None
    );
}

fn healthy(image: &NativeImage, before: &fixture::Case, row: &Row, requests: &[OmittedRowInsert]) {
    let expected = fixture::expected(before, std::slice::from_ref(row));
    checked(&before.before, 3, 2);
    checked(&expected, 4, 2);
    let original = Original::Ordinary(requests);
    let source =
        omitted::preview::qualify(image, "Items", original, &mut original::budget(), &|| false)
            .unwrap();
    let plan = stage::build(image, &source, original, &mut original::budget(), &|| false).unwrap();
    let after = compose::compose(image, &plan, &mut original::budget(), &|| false).unwrap();
    difference(&after, &expected);
    audit::prove_with(
        image,
        "Items",
        AuditRequest::Omitted(original),
        &plan,
        &after,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
}

#[test]
fn omission_audit_rejects_correlated_poisoned_expanded_cache_plan_and_candidate() {
    let source = source(6, "Score", Some("\"fallback\""));
    let mut intended = Row::rich(10);
    intended.score = Some("fallback".to_owned());
    let requests = [request(&intended, Some(13))];
    let mut alternate = intended.clone();
    alternate.score = Some("forged".to_owned());
    let expected_alternate = fixture::expected(&source, &[alternate]);
    checked(&expected_alternate, 4, 2);
    let image = original::capture(&source.before);
    healthy(&image, &source, &intended, &requests);

    let original = Original::Ordinary(&requests);
    let checked =
        omitted::preview::qualify(&image, "Items", original, &mut original::budget(), &|| {
            false
        })
        .unwrap();
    let mut cached = omitted::prepare(&image, &checked, original, &mut original::budget(), &|| {
        false
    })
    .unwrap();
    cached.rows[0].values[13] = Value::Text("forged".to_owned());
    let plan =
        stage::consume(&image, &checked, cached, &mut original::budget(), &|| false).unwrap();
    let after = compose::compose(&image, &plan, &mut original::budget(), &|| false).unwrap();
    super::checked(&after, 4, 2);
    difference(&after, &expected_alternate);
    assert_eq!(
        audit::prove_with(
            &image,
            "Items",
            AuditRequest::Omitted(original),
            &plan,
            &after,
            &mut original::budget(),
            &|| false
        )
        .err(),
        Some(EditError::Preservation)
    );
}

#[test]
fn omission_audit_never_replaces_explicit_null_with_a_correlated_default() {
    let source = source(6, "Score", Some("\"fallback\""));
    let mut intended = Row::rich(10);
    intended.score = None;
    let requests = [request(&intended, None)];
    let mut alternate = intended.clone();
    alternate.score = Some("fallback".to_owned());
    let expected_alternate = fixture::expected(&source, &[alternate]);
    checked(&expected_alternate, 4, 2);
    let image = original::capture(&source.before);
    healthy(&image, &source, &intended, &requests);

    let forged_requests = [request(&intended, Some(13))];
    let forged = Original::Ordinary(&forged_requests);
    let checked =
        omitted::preview::qualify(&image, "Items", forged, &mut original::budget(), &|| false)
            .unwrap();
    let plan = stage::build(&image, &checked, forged, &mut original::budget(), &|| false).unwrap();
    let after = compose::compose(&image, &plan, &mut original::budget(), &|| false).unwrap();
    difference(&after, &expected_alternate);
    super::checked(&after, 4, 2);
    let actual = Original::Ordinary(&requests);
    assert_eq!(
        audit::prove_with(
            &image,
            "Items",
            AuditRequest::Omitted(actual),
            &plan,
            &after,
            &mut original::budget(),
            &|| false
        )
        .err(),
        Some(EditError::Preservation)
    );
}
