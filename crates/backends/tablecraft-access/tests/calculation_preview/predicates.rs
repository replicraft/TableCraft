use super::*;
use tablecraft_expressions::EvalError;

#[test]
fn native_predicate_previews_bind_without_records_and_keep_cached_results_separate() {
    for code in [5, 6] {
        let mut bytes = calculated_fixture::database_with_expression(
            &[Some(999), None, Some(-1)],
            "([Id]<=2) And Not ([Id]=1)",
        );
        bytes[20] = code;
        let original = bytes.clone();
        let forbid_rows = Rc::new(Cell::new(true));
        let source = NativeSource::new(
            Watching {
                data: Cursor::new(bytes),
                forbid_rows: forbid_rows.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap();
        let preview = source
            .prepare_calculation("Items", "Score", ParseLimits::default())
            .unwrap();

        assert_eq!(preview.dependencies(), ["Id"]);
        assert_eq!(preview.declaration().data_type(), ScalarType::Int32);
        forbid_rows.set(false);
        assert_eq!(
            preview.execute(ExecutionOptions::default()).unwrap().rows(),
            [
                vec![Value::Boolean(false)],
                vec![Value::Boolean(true)],
                vec![Value::Boolean(false)],
            ]
        );

        let cached = read_stored_int32_calculation(
            &mut Cursor::new(original),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(cached.values(), [Some(999), None, Some(-1)]);
    }
}

#[test]
fn a_late_predicate_error_refuses_the_complete_preview_result() {
    let source = NativeSource::new(
        Cursor::new(calculated_fixture::database_with_expression(
            &[Some(999), Some(999), Some(999)],
            "([Id]=0) And (1/([Id]-2)=0)",
        )),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap();
    let preview = source
        .prepare_calculation("Items", "Score", ParseLimits::default())
        .unwrap();

    assert!(matches!(
        preview.execute(ExecutionOptions::default()),
        Err(CalculationError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
}
