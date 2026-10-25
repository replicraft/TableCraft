use super::*;
use tablecraft_expressions::EvalError;

#[path = "iif.rs"]
mod iif;

#[path = "dependencies/fixture.rs"]
mod fixture;

#[test]
fn chain_and_shared_diamond_recompute_natural_carriers_without_reading_caches() {
    for code in [5, 6] {
        for stored in [false, true] {
            let mut bytes = fixture::database(
                &[
                    ("Twice", "[Id]*2"),
                    ("Total", "[twICE]+[Id]"),
                    ("Diamond", "[Twice]+[Total]"),
                ],
                &[1, 2, 3],
            );
            bytes[20] = code;
            let forbidden = Rc::new(Cell::new(true));
            let source = NativeSource::new(
                Watching {
                    data: Cursor::new(bytes),
                    forbid_rows: forbidden.clone(),
                },
                CatalogScope::new(1),
                budget(),
                &|| false,
            )
            .unwrap();
            let source = if stored {
                source.with_stored_calculations().unwrap()
            } else {
                source
            };
            let preview = source
                .prepare_calculation("Items", "Diamond", ParseLimits::default())
                .unwrap();
            assert_eq!(preview.declaration().data_type(), ScalarType::Int32);
            assert_eq!(preview.dependencies(), ["Id"]);
            assert_eq!(preview.calculated_dependencies(), ["Twice", "Total"]);
            forbidden.set(false);
            assert_eq!(
                preview.execute(ExecutionOptions::default()).unwrap().rows(),
                &[
                    vec![Value::Int64(5)],
                    vec![Value::Int64(10)],
                    vec![Value::Int64(15)],
                ]
            );
        }
    }
}

#[test]
fn constant_alias_dependencies_do_not_load_unrelated_cached_fields() {
    let bytes = fixture::database(
        &[
            ("Constant", "7"),
            ("Alias", "[Constant]"),
            ("Unused", "Now()"),
        ],
        &[1, 2],
    );
    let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
        false
    })
    .unwrap();
    let preview = source
        .prepare_calculation("Items", "Alias", ParseLimits::default())
        .unwrap();
    assert!(preview.dependencies().is_empty());
    assert_eq!(preview.calculated_dependencies(), ["Constant"]);
    assert_eq!(
        preview.execute(ExecutionOptions::default()).unwrap().rows(),
        &[vec![Value::Int64(7)], vec![Value::Int64(7)],]
    );
}

#[test]
fn cycles_and_unsupported_transitive_inputs_refuse_before_user_rows() {
    for expressions in [
        vec![("Root", "[Root]")],
        vec![("Root", "[Other]"), ("Other", "[Root]")],
        vec![("Root", "[Other]+1"), ("Other", "Now()")],
        vec![("Root", "[Other]+1"), ("Other", "[Missing]")],
        vec![("Root", "[Other]+1"), ("Other", "[Items].[Id]")],
    ] {
        let source = NativeSource::new(
            Watching {
                data: Cursor::new(fixture::database(&expressions, &[1])),
                forbid_rows: Rc::new(Cell::new(true)),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            source.prepare_calculation("Items", "Root", ParseLimits::default()),
            Err(CalculationError::Unsupported)
        ));
    }
}

#[test]
fn dependency_chain_and_expanded_node_limits_refuse_before_rows() {
    for repeated in [false, true] {
        let count = if repeated { 12 } else { 34 };
        let owned = (0..count)
            .map(|index| {
                let formula = if index == 0 {
                    "[Id]".to_string()
                } else if repeated {
                    format!("[C{}]+[C{}]", index - 1, index - 1)
                } else {
                    format!("[C{}]", index - 1)
                };
                (format!("C{index}"), formula)
            })
            .collect::<Vec<_>>();
        let expressions = owned
            .iter()
            .map(|(name, text)| (name.as_str(), text.as_str()))
            .collect::<Vec<_>>();
        let source = NativeSource::new(
            Watching {
                data: Cursor::new(fixture::database(&expressions, &[1])),
                forbid_rows: Rc::new(Cell::new(true)),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            source.prepare_calculation("Items", &format!("C{}", count - 1), ParseLimits::default()),
            Err(CalculationError::Query(QueryError::StructureLimit))
        ));
    }
}

#[test]
fn caller_limits_apply_to_the_expanded_expression() {
    let expressions = [("A", "[Id]+1"), ("B", "[A]+1"), ("Root", "[B]+1")];
    for limits in [
        ParseLimits {
            max_depth: 3,
            ..ParseLimits::default()
        },
        ParseLimits {
            max_tokens: 6,
            ..ParseLimits::default()
        },
    ] {
        let source = NativeSource::new(
            Watching {
                data: Cursor::new(fixture::database(&expressions, &[1])),
                forbid_rows: Rc::new(Cell::new(true)),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            source.prepare_calculation("Items", "Root", limits),
            Err(CalculationError::Query(QueryError::StructureLimit))
        ));
    }
}

#[test]
fn reachable_declarations_keep_native_qualification_and_late_errors() {
    const FIRST_COLUMN: usize = 63;
    const COLUMN_BYTES: usize = 25;
    const EXTENDED_FLAGS: usize = 16;
    const OWNER_PAGE: usize = 10;
    let mut bytes = fixture::database(&[("Peer", "[Id]"), ("Root", "[Peer]+1")], &[1]);
    bytes[OWNER_PAGE * PAGE_BYTES + FIRST_COLUMN + COLUMN_BYTES + EXTENDED_FLAGS] = 0xc1;
    let source = NativeSource::new(
        Watching {
            data: Cursor::new(bytes),
            forbid_rows: Rc::new(Cell::new(true)),
        },
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        source.prepare_calculation("Items", "Root", ParseLimits::default()),
        Err(CalculationError::Native(InspectError::Unsupported))
    ));

    let source = NativeSource::new(
        Cursor::new(fixture::database(
            &[("Peer", "[Id]\\([Id]-2)"), ("Root", "[Peer]+1")],
            &[1, 2, 3],
        )),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap();
    let preview = source
        .prepare_calculation("Items", "Root", ParseLimits::default())
        .unwrap();
    assert!(matches!(
        preview.execute(ExecutionOptions::default()),
        Err(CalculationError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
}
