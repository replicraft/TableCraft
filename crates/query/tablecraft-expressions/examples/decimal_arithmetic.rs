//! Fixed arithmetic workloads for Hyperfine; no native I/O or query planning.
use clap::{Parser, ValueEnum};
use std::hint::black_box;
use tablecraft_core::values::{Decimal, Value};
use tablecraft_expressions::numeric::{Arithmetic, arithmetic};

const MAX_ITERATIONS: u64 = 100_000_000;
const LEFT: i64 = 12345;
const RIGHT: i64 = 6789;
const DIVISION_COEFFICIENT: i128 = 18183826778612461334511710119;
const OPERATORS: [Arithmetic; 3] = [Arithmetic::Add, Arithmetic::Subtract, Arithmetic::Multiply];

#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    Integer,
    Decimal,
    Division,
}
#[derive(Parser)]
struct Args {
    #[arg(long, value_enum)]
    mode: Mode,
    #[arg(long,default_value_t=1_000_000,value_parser=clap::value_parser!(u64).range(1..=MAX_ITERATIONS))]
    iterations: u64,
}
fn coefficient(value: Value) -> i128 {
    match value {
        Value::Int64(value) => i128::from(value),
        Value::Decimal(value) => value.coefficient(),
        _ => panic!("fixed arithmetic carrier"),
    }
}
fn main() {
    let args = Args::parse();
    let decimal = |value| Value::Decimal(Decimal::new(value, 0).unwrap());
    let integers = [Value::Int64(LEFT), Value::Int64(RIGHT)];
    let decimals = [decimal(i128::from(LEFT)), decimal(i128::from(RIGHT))];
    for (operator, expected) in
        OPERATORS
            .into_iter()
            .zip([LEFT + RIGHT, LEFT - RIGHT, LEFT * RIGHT])
    {
        assert_eq!(
            coefficient(arithmetic(operator, &integers[0], &integers[1]).unwrap()),
            i128::from(expected)
        );
        assert_eq!(
            coefficient(arithmetic(operator, &decimals[0], &decimals[1]).unwrap()),
            i128::from(expected)
        );
    }
    assert_eq!(
        arithmetic(Arithmetic::Divide, &decimals[0], &decimals[1]).unwrap(),
        Value::Decimal(Decimal::new(DIVISION_COEFFICIENT, 28).unwrap())
    );
    let operands = if matches!(args.mode, Mode::Integer) {
        &integers
    } else {
        &decimals
    };
    let mut checksum = 0_i128;
    for index in 0..args.iterations {
        let operator = if matches!(args.mode, Mode::Division) {
            Arithmetic::Divide
        } else {
            OPERATORS[index as usize % OPERATORS.len()]
        };
        let result = arithmetic(
            black_box(operator),
            black_box(&operands[0]),
            black_box(&operands[1]),
        )
        .unwrap();
        checksum = checksum.wrapping_add(black_box(coefficient(result)));
    }
    println!(
        "{} operations; checksum {}",
        args.iterations,
        black_box(checksum)
    );
}
