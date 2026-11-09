use super::*;
use tablecraft_core::limits::Resource;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::{
        Source as NumberSource, repeated as number_history,
    },
    support::bytes::hex,
};

#[test]
fn sealed_plans_bind_complete_source_target_and_independent_request_bytes() {
    let before = raw::source(raw::SourceCase::IdOnly);
    // The first Number oracle explicitly stores Required(false). The mixed
    // append helper expects an existing Required dictionary and cannot supply
    // this first-ADD declaration from an empty property bag.
    let first_number = number_history::source(NumberSource::IdOnly);
    assert_eq!(
        hex(&Sha256::digest(&first_number)),
        "a6a9e18cd35dc2bf018e5cd554a68b5b60d56023fd78a4a0ecba627ab9041d72"
    );
    for (target, column, expected) in [
        (
            text(255, true),
            "Label",
            raw::append_text(&before, "Label", 255, true),
        ),
        (FieldAdditionTarget::NumberLongZero, "Score", first_number),
    ] {
        let original = plan(&before, column, target);
        assert_eq!(original.before_bytes(), before);
        assert_eq!(original.after_bytes(), expected);
        assert_eq!(original.target(), target);
        assert_eq!(original.intent().target(), target);
        assert_eq!(
            original.intent().encode(&mut budget(), &|| false).unwrap(),
            literal(&before, "Table1", column, target)
        );
        let mut native = budget();
        let mut proof = budget();
        let rebound = original
            .requalify_with_budgets(image(&before), &mut native, &mut proof, &|| false)
            .unwrap();
        assert_eq!(rebound.after_bytes(), expected);
        assert!(native.remaining(Resource::Steps) < budget().remaining(Resource::Steps));
        assert!(proof.remaining(Resource::Steps) < budget().remaining(Resource::Steps));
    }
}

#[test]
fn before_digest_and_fresh_source_guards_survive_cold_request_decode() {
    let before = raw::source(raw::SourceCase::IdOnly);
    let wire = literal(&before, "Table1", "Label", text(80, true));
    let request = decode(&wire);
    request
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    let mut changed = before.clone();
    changed[4096 - 1] ^= 1;
    assert!(matches!(
        request.verify_before(&changed, &mut budget(), &|| false),
        Err(FieldAdditionIntentError::BeforeMismatch)
    ));
    assert!(matches!(
        request.verify_before(&before[..before.len() - 1], &mut budget(), &|| false),
        Err(FieldAdditionIntentError::BeforeMismatch)
    ));
    let source = plan(&before, "Label", text(80, true));
    assert!(
        source
            .requalify_with_budgets(image(&changed), &mut budget(), &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn normal_requalification_keeps_independent_native_and_proof_limits() {
    let before = raw::source(raw::SourceCase::IdOnly);
    assert!(
        plan(&before, "Label", text(255, true))
            .requalify_with_budgets(
                image(&before),
                &mut ResourceBudget::new(0, 0, 0),
                &mut budget(),
                &|| false,
            )
            .is_err()
    );
    assert!(
        plan(&before, "Label", text(255, true))
            .requalify_with_budgets(
                image(&before),
                &mut budget(),
                &mut ResourceBudget::new(0, 0, 0),
                &|| false,
            )
            .is_err()
    );
    assert!(
        plan(&before, "Label", text(255, true))
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| true,)
            .is_err()
    );
}
