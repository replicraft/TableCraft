//! Query-level timings with reusable metadata and immutable in-memory records.
use std::{hint::black_box, time::Instant};
use tablecraft_core::values::{Currency, Double, Value};
use tablecraft_query::{
    ExecutionOptions, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

const REPETITIONS: u32 = 20;

fn main() {
    println!("case,rows,groups,repetitions,nanoseconds_per_query");
    for (name, groups, per_group, data_type) in [
        ("one_group", 1_i32, 4096_i32, ScalarType::Double),
        ("many_groups", 128, 16, ScalarType::Double),
        ("currency_one_group", 1, 4096, ScalarType::Currency),
        ("currency_many_groups", 128, 16, ScalarType::Currency),
        ("integral_one_group", 1, 4096, ScalarType::Int64),
        ("integral_many_groups", 128, 16, ScalarType::Int64),
    ] {
        let rows = (0..groups)
            .flat_map(|group| {
                (0..per_group).map(move |value| {
                    vec![
                        Value::Int32(group),
                        match data_type {
                            ScalarType::Double => {
                                Value::Double(Double::from_float(f64::from(value)))
                            }
                            ScalarType::Currency => {
                                Value::Currency(Currency::from_scaled(i64::from(value) * 10000))
                            }
                            ScalarType::Int64 => Value::Int64(i64::from(value)),
                            _ => unreachable!("explicit benchmark profiles"),
                        },
                    ]
                })
            })
            .collect();
        let source = MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![
                    Column::new("Bucket".into(), ScalarType::Int32, false).unwrap(),
                    Column::new("Amount".into(), data_type, false).unwrap(),
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
        assert!(
            warm.rows()
                .iter()
                .all(|row| row[1]
                    == Value::Double(Double::from_float(f64::from(per_group - 1) / 2.0)))
        );
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
