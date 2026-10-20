//! Compare the library-backed fixed conversion with the existing integer kernel.
use clap::{Parser, ValueEnum};
use std::hint::black_box;
use tablecraft_core::values::{Decimal, Double, Value};
use tablecraft_expressions::{
    functions::conversions::Conversion,
    numeric::{IntegerType, coerce_integer},
};

const MAX_ITERATIONS: u64 = 100_000_000;
const INPUTS: [(i128, u8); 4] = [
    (123455, 5),
    (-123445, 5),
    (90071992547409931200, 8),
    (1, 28),
];
#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    Library,
    Reference,
    Floating,
}
#[derive(Parser)]
struct Args {
    #[arg(long, value_enum)]
    mode: Mode,
    #[arg(long,default_value_t=1_000_000,value_parser=clap::value_parser!(u64).range(1..=MAX_ITERATIONS))]
    iterations: u64,
}
fn reference(value: &Value) -> i64 {
    let Value::Decimal(value) = value else {
        panic!("fixed benchmark input");
    };
    let shifted = Value::Decimal(Decimal::new(value.coefficient(), value.scale() - 4).unwrap());
    let Value::Int64(result) = coerce_integer(&shifted, IntegerType::Int64).unwrap() else {
        panic!("integer result");
    };
    result
}
fn library(value: &Value) -> i64 {
    let Value::Currency(result) = Conversion::Currency.apply(value).unwrap() else {
        panic!("Currency result");
    };
    result.scaled()
}
fn main() {
    let args = Args::parse();
    let values = INPUTS
        .map(|(coefficient, scale)| Value::Decimal(Decimal::new(coefficient, scale).unwrap()));
    for value in &values {
        assert_eq!(library(value), reference(value));
    }
    let floating = [1.25, -0.03125, 549755813888.0, f64::from_bits(1)]
        .map(|value| Value::Double(Double::from_float(value)));
    for (value, expected) in floating.iter().zip([12500, -312, 5497558138880000, 0]) {
        assert_eq!(library(value), expected);
    }
    let values = if matches!(args.mode, Mode::Floating) {
        &floating
    } else {
        &values
    };
    let mut checksum = 0_i128;
    for index in 0..args.iterations {
        let value = black_box(&values[index as usize % values.len()]);
        let result = match args.mode {
            Mode::Library | Mode::Floating => library(value),
            Mode::Reference => reference(value),
        };
        checksum = checksum.wrapping_add(i128::from(black_box(result)));
    }
    println!(
        "{} conversions; checksum {}",
        args.iterations,
        black_box(checksum)
    );
}
