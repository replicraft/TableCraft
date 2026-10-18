use super::*;
use std::io::Cursor;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}
fn table() -> TableSnapshot {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    crate::read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "ROULETTE_TB",
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn implicit_field_and_explicit_table_rules_check_records_with_typed_false_results() {
    let table = table();
    let expression = compile::rule(&table, ">0 And <1000", Some(0), &mut budget(), &|| false)
        .unwrap()
        .unwrap();
    assert_eq!(
        evaluate_rows(&table, &expression, Some(0), &mut budget(), &|| false).unwrap(),
        RuleCheckStatus::Checked
    );
    let expression = compile::rule(&table, "=0", Some(0), &mut budget(), &|| false)
        .unwrap()
        .unwrap();
    assert_eq!(
        evaluate_rows(&table, &expression, Some(0), &mut budget(), &|| false),
        Err(InspectError::Constraint(ConstraintViolation::FieldRule))
    );
    let expression = compile::rule(&table, "[ORDR_NBR] > 9999", None, &mut budget(), &|| false)
        .unwrap()
        .unwrap();
    assert_eq!(
        evaluate_rows(&table, &expression, None, &mut budget(), &|| false),
        Err(InspectError::Constraint(ConstraintViolation::TableRule))
    );
}

#[test]
fn calls_host_queries_wrong_fields_and_text_collations_are_rejected_before_evaluation() {
    let table = table();
    for (rule, field) in [
        ("Len([NBR_VALUE]) > 0", None),
        ("Dangerous() = 1", None),
        ("(SELECT 1) = 1", None),
        ("[COLOR_ID] >= 0", Some(0)),
        ("[Missing] > 0", None),
        ("[Other].[ORDR_NBR] > 0", None),
        ("[NBR_VALUE] = '2'", None),
        ("1", None),
        ("1 And 2", None),
        ("Not 1", None),
    ] {
        assert!(
            compile::rule(&table, rule, field, &mut budget(), &|| false)
                .unwrap()
                .is_none(),
            "{rule}"
        );
    }
}

#[test]
fn null_predicates_remain_unqualified_and_failures_limits_cancellation_are_explicit() {
    let table = table();
    let expression = compile::rule(&table, "NULL = 1", None, &mut budget(), &|| false)
        .unwrap()
        .unwrap();
    assert_eq!(
        evaluate_rows(&table, &expression, None, &mut budget(), &|| false).unwrap(),
        RuleCheckStatus::Unsupported
    );
    let expression = compile::rule(&table, "1 / 0 > 0", None, &mut budget(), &|| false)
        .unwrap()
        .unwrap();
    assert_eq!(
        evaluate_rows(&table, &expression, None, &mut budget(), &|| false),
        Err(InspectError::Constraint(ConstraintViolation::Evaluation(
            EvalError::DivisionByZero
        )))
    );
    assert!(matches!(
        compile::rule(&table, ">0", Some(0), &mut budget(), &|| true),
        Err(InspectError::Cancelled)
    ));
    assert!(matches!(
        compile::rule(
            &table,
            ">0",
            Some(0),
            &mut ResourceBudget::new(0, 0, 100),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
    assert!(matches!(
        compile::rule(&table, &"1".repeat(4097), None, &mut budget(), &|| false),
        Err(InspectError::StructureLimit)
    ));
}
