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
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionValue, Value},
};
use tablecraft_expressions::EvalError;
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

fn equality() -> BorrowedEquality {
    BorrowedEquality {
        first: Field::Left(0),
        second: Field::Right(0),
        carrier: ScalarType::Int32,
    }
}

#[test]
fn unexpected_actual_carrier_rejects_the_entire_directory_before_matching() {
    with_runtime(|runtime| {
        let equality = equality();
        let left = vec![
            vec![CellValue::scalar(Value::Int32(1))],
            vec![CellValue::scalar(Value::Int32(2))],
        ];
        let right = vec![
            vec![CellValue::scalar(Value::Int32(1))],
            vec![CellValue::scalar(Value::Int64(2))],
        ];
        let mut matched = vec![false; right.len()];
        assert!(
            runtime
                .directory_join(&equality, &left, &right, None, (1, 1), &mut matched)
                .unwrap()
                .is_none()
        );
        assert_eq!(matched, [false, false]);
        assert!(
            runtime
                .borrowed_join_matches(&equality, &left[0], &right[0])
                .unwrap()
        );
        assert!(
            runtime
                .borrowed_join_matches(&equality, &left[1], &right[1])
                .unwrap()
        );

        let mut mixed_left = left;
        mixed_left[1][0] = CellValue::scalar(Value::Int64(2));
        assert!(
            runtime
                .directory_join(&equality, &mixed_left, &right, None, (1, 1), &mut matched)
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn unexpected_error_and_missing_key_keep_original_comparison_failures() {
    with_runtime(|runtime| {
        let equality = equality();
        let left = vec![vec![CellValue::scalar(Value::Int32(1))]];
        let right = vec![
            vec![CellValue::scalar(Value::Int32(1))],
            vec![CellValue::from(ExpressionValue::Error(7))],
        ];
        let mut matched = vec![false; right.len()];
        assert!(
            runtime
                .directory_join(&equality, &left, &right, None, (1, 1), &mut matched)
                .unwrap()
                .is_none()
        );
        assert_eq!(matched, [false, false]);
        assert_eq!(
            runtime.borrowed_join_matches(&equality, &left[0], &right[1]),
            Err(QueryError::Evaluation(EvalError::TypeMismatch))
        );

        let missing = vec![vec![]];
        assert!(
            runtime
                .directory_join(&equality, &left, &missing, None, (1, 1), &mut [false])
                .unwrap()
                .is_none()
        );
        assert_eq!(
            runtime.borrowed_join_matches(&equality, &left[0], &missing[0]),
            Err(QueryError::Width)
        );
    });
}
