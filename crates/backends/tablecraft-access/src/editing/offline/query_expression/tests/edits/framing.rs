use super::*;

#[test]
fn exact_v2_dispatch_never_retries_legacy_or_accepts_other_profiles() {
    let before = raw::two_projections();
    let wire = review(&before, true)
        .intent()
        .encode(&mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        MutationIntent::decode(&wire, &mut budget(), &|| false),
        Ok(MutationIntent::QueryExpressionEdits(_))
    ));
    let mut malformed = wire.clone();
    malformed[12] = 2;
    assert!(matches!(
        MutationIntent::decode(&malformed, &mut budget(), &|| false),
        Err(Cause::QueryExpressionEditsIntent(
            QueryExpressionEditsIntentError::UnsupportedProfile
        ))
    ));
    let mut legacy_tag = wire;
    legacy_tag[7] = b'1';
    assert!(matches!(
        MutationIntent::decode(&legacy_tag, &mut budget(), &|| false),
        Err(Cause::QueryExpressionIntent(_))
    ));
}
