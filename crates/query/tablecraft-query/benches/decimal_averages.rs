//! Ordinary Decimal query timings; widened failure cases have separate goldens.
use std::{hint::black_box, time::Instant};
use tablecraft_core::values::{Decimal, Value};
use tablecraft_expressions::functions::conversions::Conversion;
use tablecraft_query::{
    ExecutionOptions, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

const REPETITIONS: u32 = 20;

fn main() {
    println!("case,rows,groups,repetitions,nanoseconds_per_query");
    for (name, groups, per_group) in [
        ("decimal_one_group", 1_i32, 4096_i32),
        ("decimal_many_groups", 128, 16),
    ] {
        let rows = (0..groups)
            .flat_map(|group| {
                (0..per_group).map(move |value| {
                    vec![
                        Value::Int32(group),
                        Value::Decimal(Decimal::new(i128::from(value) * 25, 2).unwrap()),
                    ]
                })
            })
            .collect();
        let source = MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![
                    Column::new("Bucket".into(), ScalarType::Int32, false).unwrap(),
                    Column::new("Amount".into(), ScalarType::Decimal, false).unwrap(),
                ],
                rows,
            )
            .unwrap(),
        ])
        .unwrap();
        let parsed = parse_query(
            "SELECT Bucket,Avg(Amount) FROM T GROUP BY Bucket",
            ParseLimits::default(),
        )
        .unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let warm = plan
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(warm.rows().len(), groups as usize);
        for row in warm.rows() {
            let Value::Double(value) = Conversion::Double.apply(&row[1]).unwrap() else {
                panic!("unexpected mean");
            };
            assert_eq!(value.to_float(), f64::from(per_group - 1) / 8.0);
        }
        drop(warm);
        let start = Instant::now();
        for _ in 0..REPETITIONS {
            drop(black_box(
                plan.execute(&source, &[], ExecutionOptions::default())
                    .unwrap(),
            ));
        }
        println!(
            "{name},{},{groups},{REPETITIONS},{}",
            groups * per_group,
            start.elapsed().as_nanos() / u128::from(REPETITIONS)
        );
    }
}
