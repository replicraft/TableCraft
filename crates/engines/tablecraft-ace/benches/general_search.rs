//! Standalone search latency, including temporary key/boundary allocations/drop.
use clap::{Parser, ValueEnum};
use std::{hint::black_box, ops::Range, time::Instant};
use tablecraft_ace::{GeneralTextSearch, NativeSortOrder, find_general_text};
use tablecraft_core::limits::ResourceBudget;

const GENERAL_LCID: [u8; 2] = [9, 4];
const SEARCH_BYTES: u64 = 16 * 1024 * 1024;
const SEARCH_STEPS: u64 = 1_000_000;
const NANOS_PER_SECOND: f64 = 1_000_000_000.0;

#[derive(Clone, Copy, ValueEnum)]
enum Workload {
    Expansion,
    LongMatch,
    LongMiss,
}
#[derive(Parser)]
struct Args {
    /// Cargo supplies this marker to harness-free benchmarks.
    #[arg(long = "bench", hide = true)]
    _bench: bool,
    #[arg(value_parser=clap::value_parser!(u8).range(0..=1))]
    version: u8,
    #[arg(value_enum)]
    workload: Workload,
    #[arg(long,default_value_t=100,value_parser=clap::value_parser!(u32).range(1..=1_000_000))]
    iterations: u32,
}
fn search(order: NativeSortOrder, text: &str, needle: &str) -> Option<Range<usize>> {
    find_general_text(
        order,
        text,
        needle,
        GeneralTextSearch::ForwardFrom(0),
        &mut ResourceBudget::new(SEARCH_BYTES, 0, SEARCH_STEPS),
        &|| false,
    )
    .unwrap()
}
fn main() {
    let args = Args::parse();
    let order =
        NativeSortOrder::from_native_bytes([GENERAL_LCID[0], GENERAL_LCID[1], 0, args.version]);
    let (text, needle, expected) = match args.workload {
        Workload::Expansion => ("Straße STRASSE".into(), "ss".into(), Some(4..5)),
        Workload::LongMatch => (
            "x".repeat(1024) + "A",
            "x".repeat(1024) + "a",
            Some(0..1025),
        ),
        Workload::LongMiss => ("x".repeat(1024) + "A", "x".repeat(1024) + "b", None),
    };
    assert_eq!(search(order, &text, &needle), expected);
    let mut checksum = 0usize;
    let start = Instant::now();
    for _ in 0..args.iterations {
        let found = black_box(search(order, black_box(&text), black_box(&needle)));
        checksum = checksum.wrapping_add(found.map_or(0, |span| span.start + span.end));
    }
    println!(
        "{:.1} ns per standalone search, {} searches, checksum {}",
        start.elapsed().as_secs_f64() * NANOS_PER_SECOND / f64::from(args.iterations),
        args.iterations,
        black_box(checksum)
    );
}
