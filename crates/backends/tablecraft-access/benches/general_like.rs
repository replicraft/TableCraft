//! Complete bounded LIKE calls, excluding input construction, query planning/I/O.
use clap::{Parser, ValueEnum};
use std::{hint::black_box, time::Instant};
use tablecraft_access::GeneralCollation;
use tablecraft_ace::NativeSortOrder;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{
    logic::Truth,
    predicates::{LikeMode, like},
};

const CALL_BYTES: u64 = 16 * 1024 * 1024;
const CALL_STEPS: u64 = 1_000_000;
const MICROSECONDS_PER_SECOND: f64 = 1_000_000.0;

#[derive(Clone, Copy, ValueEnum)]
enum Workload {
    Literal,
    Class,
    Negated,
    Units,
    LongLiteral,
    Contains,
}
#[derive(Parser)]
struct Args {
    #[arg(long = "bench", hide = true)]
    _bench: bool,
    #[arg(value_parser=clap::value_parser!(u8).range(0..=1))]
    version: u8,
    #[arg(value_enum)]
    workload: Workload,
    #[arg(long,default_value_t=100,value_parser=clap::value_parser!(u32).range(1..=1_000_000))]
    iterations: u32,
}
fn evaluate(version: u8, text: &Value, pattern: &Value) -> Truth {
    let provider = GeneralCollation::new(
        NativeSortOrder::from_native_bytes([9, 4, 0, version]),
        ResourceBudget::new(CALL_BYTES, 0, CALL_STEPS),
        &|| false,
    )
    .unwrap();
    like(
        text,
        pattern,
        Some(LikeMode::Ansi89),
        Some(&provider),
        &mut ResourceBudget::new(CALL_BYTES, 0, CALL_STEPS),
        None,
    )
    .unwrap()
}
fn main() {
    let args = Args::parse();
    let (text, pattern, expected) = match args.workload {
        Workload::Literal => ("ss".into(), "ß".into(), Truth::True),
        Workload::Class => ("ss".into(), "[a-z]".into(), Truth::True),
        Workload::Negated => ("ss".into(), "[!ß]*".into(), Truth::False),
        Workload::Units => ("😀".into(), "??".into(), Truth::True),
        Workload::LongLiteral => ("x".repeat(1024) + "a", "x".repeat(1024) + "A", Truth::True),
        Workload::Contains => ("a".repeat(128), "*a*".into(), Truth::True),
    };
    let text = Value::Text(text);
    let pattern = Value::Text(pattern);
    assert_eq!(evaluate(args.version, &text, &pattern), expected);
    let mut checksum = 0usize;
    let start = Instant::now();
    for _ in 0..args.iterations {
        checksum = checksum.wrapping_add(usize::from(
            black_box(evaluate(
                args.version,
                black_box(&text),
                black_box(&pattern),
            )) == Truth::True,
        ));
    }
    println!(
        "{:.3} microseconds per standalone LIKE, {} calls, checksum {}",
        start.elapsed().as_secs_f64() * MICROSECONDS_PER_SECOND / f64::from(args.iterations),
        args.iterations,
        black_box(checksum)
    );
}
