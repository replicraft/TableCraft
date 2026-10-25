use super::*;

#[test]
fn one_hundred_thousand_generated_inputs_reduce_to_seventeen_independently_modeled_groups() {
    let source = Inputs::generated(1, 100_003);
    let expected = arithmetic::model(1, 100_003, 17);
    let rows = stream(
        arithmetic::NUMERIC_SQL,
        &source,
        ExecutionOptions {
            limits: QueryLimits {
                bytes: 128 * 1024 * 1024,
                rows: 600_000,
                steps: 50_000_000,
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(rows, expected);
    assert_eq!(rows.len(), 17);
    assert_eq!(source.visits.get(), 100_003);
    assert_eq!(source.buffered_calls.get(), 0);
    assert_eq!(*source.projections.borrow(), [vec![1, 3]]);
}
