use super::*;
use crate::{
    discovery::DiscoveryState,
    execution::{
        ExecutionOptions,
        context::GroupKeys,
        execute::{ExecutionContext, ExecutionFrame},
    },
    prepare,
    source::{MemorySource, TextCollationId, TextPolicy},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExpressionValue, Value},
};
use tablecraft_expressions::numeric::OperandDeclaration;
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
fn inactive_bridge_keeps_buffered_evaluation_and_active_bridge_copies_full_cell() {
    with_runtime(|runtime| {
        let expressions = [Expr::Field {
            text_policy: TextPolicy::Default,
            depth: 0,
            index: 0,
        }];
        let policy = TextPolicy::Column(TextCollationId::from_host(77));
        let cells = [CellValue::scalar(Value::Int32(7))
            .with_policy(policy)
            .with_complete_policy(true)
            .with_declaration(Some(OperandDeclaration::Variant))];
        let mut environment = Environment {
            row: &[],
            outer: &[],
            output: &[],
            group: None,
            collections: None,
            aggregate_results: None,
            group_keys: Some(GroupKeys {
                expressions: &expressions,
                values: &cells,
                retained_values: false,
            }),
        };
        let before = runtime.budget.remaining(Resource::Steps);
        assert!(
            runtime
                .retained_group_value(&expressions[0], environment)
                .unwrap()
                .is_none()
        );
        assert_eq!(runtime.budget.remaining(Resource::Steps), before);
        assert_eq!(
            runtime.eval(&expressions[0], environment),
            Err(QueryError::UnknownName)
        );

        environment.group_keys.as_mut().unwrap().retained_values = true;
        assert_eq!(
            runtime.eval(&expressions[0], environment).unwrap(),
            Value::Int32(7)
        );
        let actual = runtime
            .retained_expression_operand(&expressions[0], environment)
            .unwrap();
        assert_eq!(actual.value, ExpressionValue::Value(Value::Int32(7)));
        assert_eq!(actual.policy, policy);
        assert!(actual.policy_complete);
        assert_eq!(actual.declaration, Some(OperandDeclaration::Variant));
    });
}

#[test]
fn retained_key_identity_observes_the_existing_depth_limit() {
    with_runtime(|runtime| {
        let mut expression = Expr::NumericLiteral(Value::Int32(7));
        for _ in 0..=MAX_BIND_DEPTH {
            expression = Expr::CVar(Box::new(expression));
        }
        let cells = [CellValue::scalar(Value::Int32(7))];
        let environment = Environment {
            row: &[],
            outer: &[],
            output: &[],
            group: None,
            collections: None,
            aggregate_results: None,
            group_keys: Some(GroupKeys {
                expressions: std::slice::from_ref(&expression),
                values: &cells,
                retained_values: true,
            }),
        };
        assert!(matches!(
            runtime.retained_group_value(&expression, environment),
            Err(QueryError::StructureLimit)
        ));
    });
}
