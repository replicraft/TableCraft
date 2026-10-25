use super::*;
use crate::{
    discovery::DiscoveryState,
    execution::{
        ExecutionOptions,
        execute::{ExecutionContext, ExecutionFrame},
    },
    prepare,
    source::MemorySource,
};
use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;
use tablecraft_sql::parse_query;

fn with_runtime(test: impl FnOnce(&mut Runtime<'_, '_>)) {
    let source = MemorySource::new(vec![]).unwrap();
    let syntax = parse_query("SELECT 1", Default::default()).unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let discovery = DiscoveryState::new(false);
    let mut budget = ResourceBudget::new(1_000_000, 100_000, 1_000_000);
    let mut runtime = prepared
        .initialize_runtime(
            ExecutionFrame {
                source: &source,
                parameters: &[],
                options: ExecutionOptions::default(),
                discovery: &discovery,
                context: ExecutionContext {
                    strict_parameters: true,
                    repeatable_dynamic: false,
                },
            },
            &mut budget,
        )
        .unwrap();
    test(&mut runtime);
}

#[test]
fn unexpected_carrier_disables_lookup_and_keeps_every_representative_visible() {
    with_runtime(|runtime| {
        let mut lookup = runtime
            .exact_output_keys(&[crate::OutputType::scalar(ScalarType::Int32, true)])
            .unwrap();
        let mut retained: Vec<Vec<CellValue>> = Vec::new();
        for (value, expected) in [
            (Value::Int32(1), None),
            (Value::Int64(2), None),
            (Value::Int32(2), Some(1)),
        ] {
            let row = vec![CellValue::scalar(value)];
            runtime.validate_key_row(&row).unwrap();
            let (representative, fingerprint) = runtime
                .exact_key_match(
                    &mut lookup,
                    &row,
                    &retained,
                    Vec::as_slice,
                    &[TextPolicy::Default],
                )
                .unwrap();
            assert_eq!(representative, expected);
            if representative.is_none() {
                let position = retained.len();
                retained.push(row);
                if let Some(fingerprint) = fingerprint {
                    lookup
                        .as_mut()
                        .unwrap()
                        .insert(fingerprint, position, runtime.budget, None)
                        .unwrap();
                }
            }
        }
        assert!(!lookup.as_ref().unwrap().enabled);
        assert_eq!(retained.len(), 2);
    });
}

#[test]
fn unexpected_error_keeps_the_original_mixed_key_refusal() {
    with_runtime(|runtime| {
        let mut lookup = runtime
            .exact_output_keys(&[crate::OutputType::scalar(ScalarType::Int32, true)])
            .unwrap();
        let retained = vec![vec![CellValue::scalar(Value::Int32(1))]];
        let row = vec![CellValue::from(
            tablecraft_core::values::ExpressionValue::Error(7),
        )];
        runtime.validate_key_row(&row).unwrap();
        let result = runtime.exact_key_match(
            &mut lookup,
            &row,
            &retained,
            Vec::as_slice,
            &[TextPolicy::Default],
        );
        assert_eq!(
            result,
            Err(QueryError::Evaluation(
                tablecraft_expressions::EvalError::TypeMismatch
            ))
        );
        assert!(!lookup.as_ref().unwrap().enabled);
    });
}

#[test]
fn forced_fingerprint_collisions_use_the_candidate_comparator() {
    let mut budget = ResourceBudget::new(1_000_000, 100_000, 1_000_000);
    let mut directory = Directory::default();
    for representative in 0..64 {
        directory
            .insert(0, representative, &mut budget, None)
            .unwrap();
    }
    for wanted in 0..64 {
        let mut probes = 0;
        let found = directory
            .find(0, |representative, same_fingerprint| {
                check(&mut budget, None)?;
                probes += 1;
                assert!(same_fingerprint);
                Ok(representative == wanted)
            })
            .unwrap();
        assert_eq!(found, Some(wanted));
        assert!((1..=64).contains(&probes));
    }
    let absent = directory
        .find(0, |_, _| {
            check(&mut budget, None)?;
            Ok(false)
        })
        .unwrap();
    assert_eq!(absent, None);
}

#[test]
fn collision_probes_propagate_work_limits_and_cancellation() {
    let mut budget = ResourceBudget::new(1_000_000, 100_000, 1_000_000);
    let mut directory = Directory::default();
    for representative in 0..32 {
        directory
            .insert(0, representative, &mut budget, None)
            .unwrap();
    }
    let mut limited = ResourceBudget::new(1_000_000, 100_000, 1);
    let result = directory.find(0, |_, _| {
        check(&mut limited, None)?;
        Ok(false)
    });
    assert!(matches!(result, Err(QueryError::Limit(_))));

    for stop in 1..=32 {
        let observed = Cell::new(0);
        let cancelled = || {
            observed.set(observed.get() + 1);
            observed.get() == stop
        };
        let result = directory.find(0, |_, _| {
            check(&mut budget, Some(&cancelled))?;
            Ok(false)
        });
        assert_eq!(result, Err(QueryError::Cancelled));
    }
}

#[test]
fn empty_directory_is_lazy_and_failed_growth_never_refunds_budget() {
    let mut directory = Directory::default();
    let mut budget = ResourceBudget::new(0, 100, 100);
    assert_eq!(
        directory
            .find(7, |_, _| panic!("empty directory has no probes"))
            .unwrap(),
        None
    );
    assert_eq!(budget.remaining(Resource::Bytes), 0);
    let result = directory.insert(7, 0, &mut budget, None);
    assert!(matches!(result, Err(QueryError::Limit(_))));
    assert_eq!(budget.remaining(Resource::Bytes), 0);
    assert_eq!(
        directory
            .find(7, |_, _| panic!(
                "failed first allocation retained no entry"
            ))
            .unwrap(),
        None
    );
}
