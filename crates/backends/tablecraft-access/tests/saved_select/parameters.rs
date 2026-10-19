use super::*;
use tablecraft_core::values::{Double, Single, Value};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};

fn parameter<'a>(name: &'a str, kind: i16, length: Option<i32>, order: &'a [u8]) -> Fragment<'a> {
    Fragment {
        name1: Some(name),
        flag: Some(kind),
        extra: length,
        order: Some(order),
        ..row(2)
    }
}
#[test]
fn native_parameter_types_and_order_reach_the_complete_owned_query_without_binding() {
    let types = [
        (1, "BIT"),
        (2, "BYTE"),
        (3, "SHORT"),
        (4, "LONG"),
        (5, "CURRENCY"),
        (6, "IEEESINGLE"),
        (7, "IEEEDOUBLE"),
        (8, "DATETIME"),
        (9, "BINARY"),
        (10, "TEXT"),
        (11, "LONGBINARY"),
        (15, "GUID"),
    ];
    let names: Vec<_> = types
        .iter()
        .enumerate()
        .map(|(i, _)| format!("Prompt {i}?"))
        .collect();
    let keys: Vec<_> = types
        .iter()
        .enumerate()
        .map(|(i, _)| ((i + 1) as u32).to_be_bytes())
        .collect();
    let mut fragments = vec![column("1")];
    for ((name, key), (kind, _)) in names.iter().zip(&keys).zip(types) {
        fragments.push(parameter(
            name,
            kind,
            if kind == 10 { Some(17) } else { None },
            key,
        ));
    }
    let result = compile(&fragments).unwrap();
    let declarations = names
        .iter()
        .zip(types)
        .map(|(name, (kind, label))| {
            format!("[{name}] {label}{}", if kind == 10 { "(17)" } else { "" })
        })
        .collect::<Vec<_>>()
        .join(", ");
    let expected = parse_query(
        &format!("PARAMETERS {declarations}; SELECT 1"),
        Default::default(),
    )
    .unwrap();
    assert_eq!(result.query(), expected.query());
    assert_eq!(result.query().parameters.len(), 12);
    assert_eq!(result.definition().rows()[10].extra(), Some(17));
    for extra in [None, Some(0), Some(1), Some(i32::MAX)] {
        let result = compile(&[parameter("P", 10, extra, &[0, 0, 0, 1]), column("1")]).unwrap();
        let arguments = &result.query().parameters[0].data_type.arguments;
        match extra {
            Some(n) if n > 0 => assert_eq!(arguments, &[n as u32]),
            _ => assert!(arguments.is_empty()),
        }
        assert_eq!(result.definition().rows()[0].extra(), extra);
    }
}

#[test]
fn owned_native_parameters_bind_only_when_the_caller_explicitly_executes_with_exact_carriers() {
    let result = compile(&[
        parameter("S", 6, None, &[0, 0, 0, 1]),
        parameter("D", 7, None, &[0, 0, 0, 2]),
        column("[S]"),
        Fragment {
            expression: Some("[D]"),
            order: Some(&[0, 0, 0, 2]),
            ..row(6)
        },
    ])
    .unwrap();
    // Pure host control only: no native table/provider or application code runs.
    let source = MemorySource::default();
    let plan = prepare(result.query(), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::Parameter)
    );
    let single = Value::Single(Single::from_bits(0x3f80_0001));
    let double = Value::Double(Double::from_bits(0x3ff0_0000_0000_0001));
    let values = plan
        .execute(
            &source,
            &[("S", single.clone()), ("D", double.clone())],
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(values.rows(), &[vec![single, double.clone()]]);
    assert_eq!(
        plan.execute(
            &source,
            &[("S", double.clone()), ("D", double)],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Parameter)
    );
}

#[test]
fn parameter_metadata_does_not_default_unknown_types_lengths_names_or_order() {
    for kind in [0, 12, 13, 16, 17, i16::MIN] {
        assert_eq!(
            compile(&[parameter("P", kind, None, &[0, 0, 0, 1]), column("1")]).err(),
            Some(E::Unsupported)
        );
    }
    for (kind, extra) in [(10, -1), (10, i32::MIN), (4, 0), (6, 1)] {
        assert_eq!(
            compile(&[
                parameter("P", kind, Some(extra), &[0, 0, 0, 1]),
                column("1")
            ])
            .err(),
            Some(E::Unsupported)
        );
    }
    for name in ["", "P.Q", "Forms!P", "[P]", "é", "P\nQ"] {
        assert_eq!(
            compile(&[parameter(name, 4, None, &[0, 0, 0, 1]), column("1")]).err(),
            Some(E::Unsupported)
        );
    }
    assert_eq!(
        compile(&[
            parameter("P", 4, None, &[0, 0, 0, 1]),
            parameter("p", 4, None, &[0, 0, 0, 2]),
            column("1")
        ])
        .err(),
        Some(E::InvalidMetadata)
    );
    for kind in 0..5 {
        let mut bad = parameter("P", 4, None, &[0, 0, 0, 1]);
        match kind {
            0 => bad.name1 = None,
            1 => bad.flag = None,
            2 => bad.expression = Some("NeverRunMe()"),
            3 => bad.name2 = Some("unused"),
            _ => bad.order = None,
        }
        assert!(compile(&[bad, column("1")]).is_err());
    }
    assert_eq!(
        compile(&[
            parameter("P", 4, None, &[0, 0, 0, 2]),
            parameter("Q", 4, None, &[0, 0, 0, 1]),
            column("1")
        ])
        .err(),
        Some(E::InvalidMetadata)
    );
}

#[test]
fn declaration_ceiling_budgets_and_late_cancellation_precede_model_publication() {
    let names: Vec<_> = (0..256).map(|i| format!("P{i}")).collect();
    let keys: Vec<_> = (1..=256_u32).map(u32::to_be_bytes).collect();
    for count in [255, 256] {
        let mut fragments: Vec<_> = names
            .iter()
            .zip(&keys)
            .take(count)
            .map(|(name, key)| parameter(name, 2, None, key))
            .collect();
        fragments.push(column("1"));
        let mut allowed = ResourceBudget::new(256 * 1024 * 1024, 16_384, 1_000_000);
        let result = reconstruct_saved_select(
            definition(&fragments, 0),
            Default::default(),
            &mut allowed,
            &|| false,
        );
        if count == 255 {
            assert_eq!(result.unwrap().query().parameters.len(), count);
        } else {
            assert_eq!(result.err(), Some(E::SyntaxLimit));
        }
    }
    let fragments = [
        parameter("Prompt", 10, Some(12), &[0, 0, 0, 1]),
        column("1"),
    ];
    let checks = Cell::new(0);
    reconstruct_saved_select(
        definition(&fragments, 0),
        Default::default(),
        &mut budget(),
        &|| {
            checks.set(checks.get() + 1);
            false
        },
    )
    .unwrap();
    let final_check = checks.get();
    checks.set(0);
    assert_eq!(
        reconstruct_saved_select(
            definition(&fragments, 0),
            Default::default(),
            &mut budget(),
            &|| {
                checks.set(checks.get() + 1);
                checks.get() == final_check
            }
        )
        .err(),
        Some(E::Cancelled)
    );
    assert!(matches!(
        reconstruct_saved_select(
            definition(&fragments, 0),
            Default::default(),
            &mut ResourceBudget::new(1024, 100, 1_000_000),
            &|| false
        ),
        Err(E::Limit(_))
    ));
}
