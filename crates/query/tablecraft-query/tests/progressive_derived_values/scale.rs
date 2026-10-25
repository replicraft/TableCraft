use super::*;

#[test]
fn one_hundred_thousand_generated_rows_flow_through_three_named_stages_into_exact_aggregates() {
    let source = Inputs::new(
        100_003,
        &[
            ("A", "SELECT Id,Score FROM Data WHERE Id Mod 3=0"),
            ("B", "SELECT Id AS K,Score FROM A WHERE Id Mod 5<>0"),
            ("C", "SELECT K,Score FROM B WHERE K>=17"),
        ],
    );
    let mut expected_count = 0_i64;
    let mut expected_sum = 0_i64;
    for id in 1..=100_003 {
        if id % 3 == 0 && id % 5 != 0 && id >= 17 {
            expected_count += 1;
            expected_sum += i64::from(id) * 3;
        }
    }
    let (summary, rows) = run(
        "SELECT Count(*),Sum(Score) FROM C",
        &source,
        &[],
        ExecutionOptions {
            limits: QueryLimits {
                bytes: 128 * 1024 * 1024,
                rows: 800_000,
                steps: 50_000_000,
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        rows,
        [vec![
            Value::Int64(expected_count),
            Value::Int64(expected_sum)
        ]]
    );
    assert_eq!(summary.scanned_rows, 100_003);
    assert_eq!(summary.emitted_rows, 1);
    assert_eq!(source.visits.get(), 100_003);
    assert_eq!(*source.scans.borrow(), ["Data"]);
    assert_eq!(*source.projections.borrow(), [vec![1, 2]]);
}
