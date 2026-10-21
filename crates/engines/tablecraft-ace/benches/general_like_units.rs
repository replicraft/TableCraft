//! FST/BTree/sorted-vector lookup alternatives and separate complete classification.
use clap::{Parser, ValueEnum};
use fst::Streamer;
use std::{collections::BTreeSet, hint::black_box, time::Instant};
use tablecraft_ace::{NativeSortOrder, is_general_like_unit};
use tablecraft_core::limits::ResourceBudget;

const SAMPLE_STRIDE: usize = 1024;
const NANOS_PER_SECOND: f64 = 1_000_000_000.0;
const CALL_BYTES: u64 = 16 * 1024 * 1024;
const CALL_STEPS: u64 = 1_000_000;

#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    Fst,
    Btree,
    Sorted,
    Classifier,
}
#[derive(Parser)]
struct Args {
    #[arg(long = "bench", hide = true)]
    _bench: bool,
    #[arg(value_parser=clap::value_parser!(u8).range(0..=1))]
    version: u8,
    #[arg(value_enum)]
    mode: Mode,
    #[arg(long,default_value_t=1000,value_parser=clap::value_parser!(u32).range(1..=1_000_000))]
    iterations: u32,
}
fn main() {
    let args = Args::parse();
    let bytes: &[u8] = if args.version == 0 {
        include_bytes!("../data/general/like-legacy.fst")
    } else {
        include_bytes!("../data/general/like-2010.fst")
    };
    let dictionary = fst::Set::new(bytes).unwrap();
    dictionary.as_fst().verify().unwrap();
    let mut probes = Vec::new();
    let mut owned = Vec::new();
    let mut tree = BTreeSet::new();
    let mut stream = dictionary.stream();
    let mut count = 0;
    while let Some(key) = stream.next() {
        if count % SAMPLE_STRIDE == 0 {
            probes.push((key.to_vec(), true));
            let mut absent = key.to_vec();
            absent.push(u8::MAX);
            while dictionary.contains(&absent) {
                absent.push(u8::MAX);
            }
            probes.push((absent, false));
        }
        if matches!(args.mode, Mode::Sorted) {
            owned.push(key.to_vec());
        }
        if matches!(args.mode, Mode::Btree) {
            tree.insert(key.to_vec());
        }
        count += 1;
    }
    let corpus: Vec<Vec<u16>> = ["a", "A", "ss", "ae", "é", "tablecraft"]
        .iter()
        .map(|s| s.encode_utf16().collect())
        .collect();
    let order = NativeSortOrder::from_native_bytes([9, 4, 0, args.version]);
    let lookup = |value: &[u8]| match args.mode {
        Mode::Fst => dictionary.contains(value),
        Mode::Btree => tree.contains(value),
        Mode::Sorted => owned
            .binary_search_by(|key| key.as_slice().cmp(value))
            .is_ok(),
        Mode::Classifier => unreachable!(),
    };
    let classify = |units: &[u16]| {
        is_general_like_unit(
            order,
            units,
            &mut ResourceBudget::new(CALL_BYTES, 0, CALL_STEPS),
            &|| false,
        )
        .unwrap()
    };
    if matches!(args.mode, Mode::Classifier) {
        for (index, units) in corpus.iter().enumerate() {
            assert_eq!(classify(units), index + 1 < corpus.len());
        }
    } else {
        for (key, expected) in &probes {
            assert_eq!(lookup(key), *expected);
        }
    }
    let mut checksum = 0usize;
    let start = Instant::now();
    for _ in 0..args.iterations {
        if matches!(args.mode, Mode::Classifier) {
            for units in &corpus {
                checksum =
                    checksum.wrapping_add(usize::from(black_box(classify(black_box(units)))));
            }
        } else {
            for (key, _) in &probes {
                checksum = checksum.wrapping_add(usize::from(black_box(lookup(black_box(key)))));
            }
        }
    }
    let operations = u64::from(args.iterations)
        * if matches!(args.mode, Mode::Classifier) {
            corpus.len()
        } else {
            probes.len()
        } as u64;
    println!(
        "{:.1} ns per operation, {operations} operations, {count} distinct keys, {} static FST bytes, checksum {}",
        start.elapsed().as_secs_f64() * NANOS_PER_SECOND / operations as f64,
        bytes.len(),
        black_box(checksum)
    );
}
