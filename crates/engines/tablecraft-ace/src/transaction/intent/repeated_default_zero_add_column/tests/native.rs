use super::*;

#[test]
fn original_intents_and_fresh_plans_match_all_three_independent_sdk_afters() {
    for (case, table, column) in [
        (Source::IdOnly, "Table1", "Score2"),
        (Source::GuiRequiredZero, "Table1", "Extra2"),
        (Source::DefaultOnly, "Items", "Extra2"),
    ] {
        let before = raw::source(case);
        let after = raw::append(&before, column);
        let reviewed = plan(&before, table, column);
        let expected = literal(&before, table, column);
        assert_eq!(
            reviewed.intent().encode(&mut budget(), &|| false).unwrap(),
            expected
        );
        assert_eq!(
            reviewed.required_property(),
            CreatedDefaultZeroColumnRequired::StoredFalse
        );
        let fresh = reviewed
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(fresh.before_bytes(), before);
        assert_eq!(fresh.after_bytes(), after);
        assert_eq!(fresh.before_bytes().len(), fresh.after_bytes().len());
        assert_eq!(decode(&expected).table(), table);
        assert_eq!(decode(&expected).column(), column);
    }
}

#[test]
fn request_source_and_binding_changes_never_requalify_as_reviewed_images() {
    let before = raw::source(Source::IdOnly);
    let mut changed = before.clone();
    *changed.last_mut().unwrap() ^= 1;
    assert!(matches!(
        plan(&before, "Table1", "Score2").requalify_with_budgets(
            image(&changed),
            &mut budget(),
            &mut budget(),
            &|| false
        ),
        Err(RepeatedDefaultZeroAddColumnIntentError::ImageMismatch)
    ));
    let mut requested = plan(&before, "Table1", "Score2");
    requested.intent.column = copy_name("Other2", &mut budget(), &|| false).unwrap();
    assert!(
        requested
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
            .is_err()
    );
    let mut bound = plan(&before, "Table1", "Score2");
    bound.intent.before_binding[0] ^= 1;
    assert!(matches!(
        bound.requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false),
        Err(RepeatedDefaultZeroAddColumnIntentError::BeforeMismatch)
    ));
    let fresh_creator =
        tablecraft_testkit::fixtures::access::canonical_created_number::source(Source::IdOnly);
    assert!(
        image(&fresh_creator)
            .plan_recoverable_repeated_default_zero_int32_column(
                "Table1",
                "Score2",
                &mut budget(),
                &|| false
            )
            .is_err()
    );
}

#[test]
fn separate_native_proof_pools_and_cancellation_never_create_authority() {
    let before = raw::source(Source::IdOnly);
    for native_empty in [false, true] {
        let mut native = if native_empty {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let mut proof = if native_empty {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        assert!(
            plan(&before, "Table1", "Score2")
                .requalify_with_budgets(image(&before), &mut native, &mut proof, &|| false)
                .is_err()
        );
    }
    assert!(
        plan(&before, "Table1", "Score2")
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| true)
            .is_err()
    );
}
