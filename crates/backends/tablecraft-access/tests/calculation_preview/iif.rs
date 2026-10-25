use super::*;

#[test]
fn direct_and_transitive_iif_recompute_every_dependency_without_cache_inputs() {
    for code in [5, 6] {
        let mut bytes = fixture::database(
            &[
                ("Choice", "iIf([Id]>1,[Id]*2,0)"),
                ("Root", "IIf([Id]>2,[Choice]+1,IIf(NULL,99,[Choice]))"),
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
        let direct = source
            .prepare_calculation("Items", "Choice", ParseLimits::default())
            .unwrap();
        let root = source
            .prepare_calculation("Items", "Root", ParseLimits::default())
            .unwrap();
        assert_eq!(root.dependencies(), ["Id"]);
        assert_eq!(root.calculated_dependencies(), ["Choice"]);
        assert_eq!(root.declaration().data_type(), ScalarType::Int32);

        forbidden.set(false);
        assert_eq!(
            direct.execute(ExecutionOptions::default()).unwrap().rows(),
            &[
                vec![Value::Int64(0)],
                vec![Value::Int64(4)],
                vec![Value::Int64(6)]
            ]
        );
        assert_eq!(
            root.execute(ExecutionOptions::default()).unwrap().rows(),
            &[
                vec![Value::Int64(0)],
                vec![Value::Int64(4)],
                vec![Value::Int64(7)]
            ]
        );
    }
}

#[test]
fn selected_iif_branches_keep_natural_carriers_and_unselected_calculations_ignore_caches() {
    let source = NativeSource::new(
        Cursor::new(fixture::database(
            &[
                ("Choice", "[Id]*2"),
                ("Mixed", "IIf([Id]=1,[Id],'other')"),
                ("Unused", "IIf(True,[Id],[Choice])"),
            ],
            &[1, 2],
        )),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap();
    let mixed = source
        .prepare_calculation("Items", "Mixed", ParseLimits::default())
        .unwrap();
    assert_eq!(mixed.declaration().data_type(), ScalarType::Int32);
    assert_eq!(
        mixed.execute(ExecutionOptions::default()).unwrap().rows(),
        &[vec![Value::Int32(1)], vec![Value::Text("other".into())]]
    );

    // The fixture's Choice wrapper is corrupt, including on rows whose
    // condition selects the ordinary Id arm. Only its formula is recomputed.
    let unused = source
        .prepare_calculation("Items", "Unused", ParseLimits::default())
        .unwrap();
    assert_eq!(unused.calculated_dependencies(), ["Choice"]);
    assert_eq!(
        unused.execute(ExecutionOptions::default()).unwrap().rows(),
        &[vec![Value::Int32(1)], vec![Value::Int32(2)]]
    );
}

#[test]
fn invalid_calls_and_hidden_dependencies_refuse_metadata_only_even_on_empty_tables() {
    for identities in [&[][..], &[1][..]] {
        for formula in [
            "IIf(True,[Id])",
            "IIf(True,[Id],0,1)",
            "IIf(True,,0)",
            "IIf(True,*,0)",
            "VBA.IIf(True,[Id],0)",
            "IIf$(True,[Id],0)",
            "IIf(True,[Id],[Missing])",
            "IIf(True,[Id],[Root])",
            "IIf(True,[Id],Date())",
            "IIf(True,[Id],IsNull([Id]))",
            "IIf(True,[Id],Len('a'))",
            "IIf(True,[Id],IIf(False,Now(),0))",
            "IIf(True,[Id],[Other])",
        ] {
            let source = NativeSource::new(
                Watching {
                    data: Cursor::new(fixture::database(
                        &[("Root", formula), ("Other", "IIf(False,0,[Root])")],
                        identities,
                    )),
                    forbid_rows: Rc::new(Cell::new(true)),
                },
                CatalogScope::new(1),
                budget(),
                &|| false,
            )
            .unwrap();
            assert!(
                source
                    .prepare_calculation("Items", "Root", ParseLimits::default())
                    .is_err(),
                "{formula}"
            );
        }
    }
}

#[test]
fn inactive_arm_late_failure_is_eager_and_never_returns_a_partial_result() {
    let source = NativeSource::new(
        Cursor::new(fixture::database(
            &[("Root", "IIf([Id]>0,[Id],1/([Id]-3))")],
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

#[test]
fn all_iif_arguments_retain_expansion_limits_cancellation_and_execution_quotas() {
    let cancelled = Cell::new(false);
    let cancellation = || cancelled.get();
    let source = NativeSource::new(
        Watching {
            data: Cursor::new(fixture::database(
                &[("A", "[Id]+1"), ("Root", "IIf(True,[Id],[A]+1)")],
                &[1],
            )),
            forbid_rows: Rc::new(Cell::new(true)),
        },
        CatalogScope::new(1),
        budget(),
        &cancellation,
    )
    .unwrap();
    for limits in [
        ParseLimits {
            max_depth: 3,
            ..ParseLimits::default()
        },
        ParseLimits {
            max_tokens: 8,
            ..ParseLimits::default()
        },
    ] {
        assert!(matches!(
            source.prepare_calculation("Items", "Root", limits),
            Err(CalculationError::Query(QueryError::StructureLimit))
                | Err(CalculationError::Parse(_))
        ));
    }
    let preview = source
        .prepare_calculation("Items", "Root", ParseLimits::default())
        .unwrap();
    cancelled.set(true);
    assert!(matches!(
        source.prepare_calculation("Items", "Root", ParseLimits::default()),
        Err(CalculationError::Query(QueryError::Cancelled))
            | Err(CalculationError::Native(InspectError::Cancelled))
    ));
    assert!(matches!(
        preview.execute(ExecutionOptions::default()),
        Err(CalculationError::Query(QueryError::Cancelled))
    ));

    cancelled.set(false);
    let options = ExecutionOptions {
        limits: QueryLimits {
            steps: 1,
            ..QueryLimits::default()
        },
        ..ExecutionOptions::default()
    };
    assert!(preview.execute(options).is_err());
}
