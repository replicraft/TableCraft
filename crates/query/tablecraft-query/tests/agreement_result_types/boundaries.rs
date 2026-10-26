//! Validate inferred successful carriers against independently selected edge values.
use super::*;
use tablecraft_core::values::{LegacyDateTime, Single};

fn samples() -> Vec<(ScalarType, Vec<Value>)> {
    vec![
        (ScalarType::Byte, [0, 1, 255].map(Value::Byte).to_vec()),
        (
            ScalarType::Int16,
            [i16::MIN, -1, 0, 1, i16::MAX].map(Value::Int16).to_vec(),
        ),
        (
            ScalarType::Int32,
            [i32::MIN, -32769, -1, 0, 1, 32768, i32::MAX]
                .map(Value::Int32)
                .to_vec(),
        ),
        (
            ScalarType::Single,
            [-f32::MAX, -1.5, -0.0, f32::from_bits(1), 1.5, f32::MAX]
                .map(|value| Value::Single(Single::from_float(value)))
                .to_vec(),
        ),
        (
            ScalarType::DateTime,
            [-657434.75, -1.5, -0.0, 1.5, 2958465.75]
                .map(|value| Value::DateTime(LegacyDateTime::from_ole_days(value).unwrap()))
                .to_vec(),
        ),
    ]
}

fn source(left: ScalarType, right: ScalarType, a: Value, b: Value) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("A".into(), left, true).unwrap(),
                Column::new("B".into(), right, true).unwrap(),
            ],
            vec![vec![a, b]],
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn successful_edge_results_match_unknown_operand_carrier_guarantees() {
    let mut successful = 0;
    let mut refused = 0;
    for (left, left_values) in samples() {
        for (right, right_values) in samples() {
            let schema = source(left, right, Value::Null, Value::Null);
            for operator in ["+", "-", "*", "/", "\\", "Mod", "^"] {
                let sql = format!("SELECT First(A){operator} First(B) AS V FROM T");
                let syntax = parse_query(&sql, Default::default()).unwrap();
                let plan = prepare_with_options(
                    syntax.query(),
                    &schema,
                    PreparationOptions {
                        binding_depth: BindingDepth::new(32).unwrap(),
                        ..Default::default()
                    },
                )
                .unwrap();
                for a in left_values.iter().chain([&Value::Null]) {
                    for b in right_values.iter().chain([&Value::Null]) {
                        let data = source(left, right, a.clone(), b.clone());
                        let result = plan.execute(
                            &data,
                            &[],
                            ExecutionOptions {
                                declared_integral: true,
                                ..Default::default()
                            },
                        );
                        let Ok(result) = result else {
                            refused += 1;
                            continue;
                        };
                        successful += 1;
                        if let OutputType::Scalar { data_type, .. } = result.output_types()[0] {
                            let actual = &result.rows()[0][0];
                            assert!(
                                matches!(actual, Value::Null)
                                    || ScalarType::of(actual) == Some(data_type.scalar_type()),
                                "{sql}, {left:?}, {right:?}"
                            );
                        }
                    }
                }
            }
        }
    }
    // Both ordinary values and failing width/zero/range edges must be exercised.
    assert!(successful > 3000);
    assert!(refused > 100);
}
