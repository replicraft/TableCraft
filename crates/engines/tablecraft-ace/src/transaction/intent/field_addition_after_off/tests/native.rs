use super::*;
use tablecraft_core::limits::Resource;

#[test]
fn sealed_originals_bind_all_four_independent_images_and_wire_requests() {
    for case in [raw::SourceCase::Number, raw::SourceCase::Mixed] {
        let before = raw::source(case);
        for (column, target, expected) in [
            (
                "AfterOff",
                FieldAdditionTarget::NumberLongZero,
                raw::append_number(&before, "AfterOff"),
            ),
            ("LabelOff", text(), raw::append_text(&before, "LabelOff")),
        ] {
            let original = plan(&before, column, target);
            assert_eq!(original.before_bytes(), before);
            assert_eq!(original.after_bytes(), expected);
            assert_eq!(original.target(), target);
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
}

#[test]
fn original_binding_and_independent_normal_pools_are_mandatory() {
    let before = raw::source(raw::SourceCase::Number);
    let wire = literal(
        &before,
        "Table1",
        "AfterOff",
        FieldAdditionTarget::NumberLongZero,
    );
    let request = decode(&wire);
    let mut stale = before.clone();
    stale[7 * PAGE + 3000] ^= 1;
    for changed in [&stale[..], &before[..before.len() - 1]] {
        assert!(matches!(
            request.verify_before(changed, &mut budget(), &|| false),
            Err(FieldAdditionAfterOffIntentError::BeforeMismatch)
        ));
    }
    for empty_native in [false, true] {
        let mut native = if empty_native {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let mut proof = if empty_native {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        assert!(
            plan(&before, "AfterOff", FieldAdditionTarget::NumberLongZero)
                .requalify_with_budgets(image(&before), &mut native, &mut proof, &|| false)
                .is_err()
        );
    }
    assert!(
        plan(&before, "AfterOff", FieldAdditionTarget::NumberLongZero)
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| true)
            .is_err()
    );
}
