//! End-to-end sum timings, with setup/binding excluded and result drop included.
use std::{hint::black_box, time::Instant};
use tablecraft_core::values::{Double, Value};
use tablecraft_query::{
    ExecutionOptions, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

const REPETITIONS: u32 = 20;

fn main() {
    println!("case,rows,groups,repetitions,nanoseconds_per_query");
    for (name, groups, per_group, floating) in [
        ("floating_one_group", 1_i32, 4096_i32, true),
        ("floating_many_groups", 128, 16, true),
        ("integral_one_group", 1, 4096, false),
    ] {
        let rows = (0..groups)
            .flat_map(|group| {
                (0..per_group).map(move |value| {
                    vec![
                        Value::Int32(group),
                        if floating {
                            Value::Double(Double::from_float(f64::from(value)))
                        } else {
                            Value::Int64(i64::from(value))
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
                    Column::new(
                        "Amount".into(),
                        if floating {
                            ScalarType::Double
                        } else {
                            ScalarType::Int64
                        },
                        false,
                    )
                    .unwrap(),
                ],
                rows,
            )
            .unwrap(),
        ])
        .unwrap();
        let parsed = parse_query(
            "SELECT Bucket,Sum(Amount) FROM T GROUP BY Bucket",
            Default::default(),
        )
        .unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let warm = plan
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        let total = i64::from(per_group) * i64::from(per_group - 1) / 2;
        let expected = if floating {
            Value::Double(Double::from_float(total as f64))
        } else {
            Value::Int64(total)
        };
        assert_eq!(warm.rows().len(), groups as usize);
        assert!(warm.rows().iter().all(|row| row[1] == expected));
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
