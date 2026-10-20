//! Owned text-key allocation, encoding and destruction over a fixed mixed corpus.
use clap::Parser;
use std::{hint::black_box, time::Instant};
use tablecraft_ace::{NativeSortOrder, encode_text_index_key};
use tablecraft_core::limits::ResourceBudget;

const LCID_GENERAL_BYTES: [u8; 2] = [9, 4];
const CALL_BYTES: u64 = 1024 * 1024;
const CALL_STEPS: u64 = 100_000;
const NANOS_PER_SECOND: f64 = 1_000_000_000.0;

#[derive(Parser)]
struct Args {
    /// Stored General weight version: 0 is legacy, 1 is General 2010.
    #[arg(value_parser = clap::value_parser!(u8).range(0..=1))]
    version: u8,
    #[arg(long, default_value_t = 10000, value_parser = clap::value_parser!(u32).range(1..=10_000_000))]
    iterations: u32,
}
fn main() {
    let args = Args::parse();
    let order = NativeSortOrder::from_native_bytes([
        LCID_GENERAL_BYTES[0],
        LCID_GENERAL_BYTES[1],
        0,
        args.version,
    ]);
    let corpus = [
        "TableCraft",
        "Résumé",
        "e\u{301}cole",
        "Straße",
        "漢字々",
        "カタカナー",
        "\u{6d6}\u{6d6}\u{6d6}",
        "😀",
    ];
    let mut checksum = 0usize;
    let started = Instant::now();
    for _ in 0..args.iterations {
        for text in corpus {
            let mut budget = ResourceBudget::new(CALL_BYTES, 0, CALL_STEPS);
            let key = encode_text_index_key(order, black_box(text), false, &mut budget, &|| false)
                .expect("fixed qualified corpus");
            checksum = checksum.wrapping_add(black_box(key.as_bytes()).len());
        }
    }
    let elapsed = started.elapsed();
    let operations = u64::from(args.iterations) * corpus.len() as u64;
    println!(
        "{:.1} ns per owned key, {operations} keys, checksum {}",
        elapsed.as_secs_f64() * NANOS_PER_SECOND / operations as f64,
        black_box(checksum)
    );
}
