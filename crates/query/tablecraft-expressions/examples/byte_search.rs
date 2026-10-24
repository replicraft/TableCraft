//! Byte-search experiment only; does not alter production InStrB work accounting.
#[path = "byte_search/algorithm.rs"]
mod algorithm;

use algorithm::{Meter, Search};
use clap::{Parser, ValueEnum};
use std::{hint::black_box, process::ExitCode};

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_NEEDLE_BYTES: usize = 512;
const MAX_ITERATIONS: u32 = 1_000_000;
const FAILURE_STATUS: u8 = 2;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Algorithm {
    Scalar,
    Bounded,
    Reference,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Scenario {
    Front,
    Late,
    Absent,
    Random,
    Raw,
}

#[derive(Parser)]
struct Arguments {
    #[arg(long, value_enum)]
    algorithm: Algorithm,
    #[arg(long, value_enum)]
    scenario: Scenario,
    #[arg(long, default_value_t = 65_536)]
    bytes: usize,
    #[arg(long, default_value_t = 16)]
    needle_bytes: usize,
    #[arg(long, default_value_t = 100)]
    iterations: u32,
}

fn inputs(args: &Arguments) -> Result<(Vec<u8>, Vec<u8>), &'static str> {
    if args.bytes == 0
        || args.bytes > MAX_INPUT_BYTES
        || args.needle_bytes == 0
        || args.needle_bytes > MAX_NEEDLE_BYTES
        || args.needle_bytes > args.bytes
        || args.iterations == 0
        || args.iterations > MAX_ITERATIONS
    {
        return Err("input or iteration range exceeded");
    }
    let mut haystack = vec![b'a'; args.bytes];
    let mut needle = vec![b'a'; args.needle_bytes];
    *needle.last_mut().ok_or("empty needle")? = b'b';
    match args.scenario {
        Scenario::Front => haystack[..needle.len()].copy_from_slice(&needle),
        Scenario::Late => {
            let start = haystack.len() - needle.len();
            haystack[start..].copy_from_slice(&needle);
        }
        Scenario::Absent => {}
        Scenario::Random => {
            let mut state = 0x4992_3815_2817_ae61_u64;
            for byte in &mut haystack {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = state as u8;
            }
            let start = (haystack.len() - needle.len()) / 2;
            needle.copy_from_slice(&haystack[start..start + args.needle_bytes]);
        }
        Scenario::Raw => {
            let pattern = [0x00, 0xd8, 0x61, 0x00, 0xff, 0x3e, 0xd8];
            for (position, byte) in haystack.iter_mut().enumerate() {
                *byte = pattern[position % pattern.len()];
            }
            needle.fill(0xff);
            let start = (haystack.len() - needle.len()) / 2;
            haystack[start..start + needle.len()].copy_from_slice(&needle);
        }
    }
    Ok((haystack, needle))
}

fn run(args: Arguments) -> Result<(), &'static str> {
    let (haystack, needle) = inputs(&args)?;
    let search: Search = match args.algorithm {
        Algorithm::Scalar => algorithm::scalar,
        Algorithm::Bounded => algorithm::bounded,
        Algorithm::Reference => algorithm::reference,
    };
    let mut meter = Meter::default();
    let first = search(&haystack, &needle, &mut meter).map_err(|_| "search failed")?;
    let mut checksum = 0_usize;
    for _ in 0..args.iterations {
        let result = search(
            black_box(&haystack),
            black_box(&needle),
            &mut Meter::default(),
        )
        .map_err(|_| "search failed")?;
        checksum = checksum.wrapping_add(result.map_or(0, |offset| offset + 1));
    }
    black_box(checksum);
    println!(
        "{:?}\t{:?}\t{}\t{}\t{}\t{:?}\t{}\t{}\t{}",
        args.algorithm,
        args.scenario,
        haystack.len(),
        needle.len(),
        args.iterations,
        first,
        meter.work,
        meter.checkpoints,
        checksum,
    );
    Ok(())
}

fn main() -> ExitCode {
    match run(Arguments::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(FAILURE_STATUS)
        }
    }
}
