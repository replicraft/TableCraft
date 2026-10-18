//! Absolute costs of complete bounded attachment decoding, not Access comparisons.
use clap::{Parser, ValueEnum};
use flate2::{Compression, write::ZlibEncoder};
use std::{hint::black_box, io::Write, time::Instant};
use tablecraft_ace::decode_attachment;
use tablecraft_core::limits::ResourceBudget;

const ITERATIONS: u32 = 2_000;
const FILE_BYTES: usize = 60_000;
const BYTE_BUDGET: u64 = 2 * 1024 * 1024;
const STEP_BUDGET: u64 = 2_000_000;
const MICROS_PER_SECOND: f64 = 1_000_000.0;
const BYTES_PER_MIB: f64 = 1024.0 * 1024.0;

#[derive(Clone, Copy, Eq, PartialEq, ValueEnum)]
enum Case {
    StoredRepeated,
    ZlibRepeated,
    StoredNoise,
    ZlibNoise,
}

#[derive(Parser)]
struct Arguments {
    #[arg(long,default_value_t=ITERATIONS,value_parser=clap::value_parser!(u32).range(1..=100_000))]
    iterations: u32,
    #[arg(long, hide = true)]
    bench: bool,
    #[arg(long, value_enum)]
    case: Option<Case>,
}
fn wrapper(data: &[u8], compressed: bool) -> Vec<u8> {
    let mut body = vec![
        20, 0, 0, 0, 1, 0, 0, 0, 4, 0, 0, 0, b'b', 0, b'i', 0, b'n', 0, 0, 0,
    ];
    body.extend(data);
    let mut encoded = u32::from(compressed).to_le_bytes().to_vec();
    encoded.extend((body.len() as u32).to_le_bytes());
    if compressed {
        let mut stream = ZlibEncoder::new(Vec::new(), Compression::fast());
        stream.write_all(&body).unwrap();
        encoded.extend(stream.finish().unwrap());
    } else {
        encoded.extend(body);
    }
    encoded
}
fn allowance() -> ResourceBudget {
    ResourceBudget::new(BYTE_BUDGET, 0, STEP_BUDGET)
}
fn main() {
    let args = Arguments::parse();
    let repeated = vec![42; FILE_BYTES];
    let mut state = 0x41545441_u32;
    let noise = (0..FILE_BYTES)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect::<Vec<_>>();
    println!("case\tfile bytes\tmicroseconds/payload\tMiB/s");
    for (case, name, data, compressed) in [
        (Case::StoredRepeated, "stored/repeated", &repeated, false),
        (Case::ZlibRepeated, "zlib/repeated", &repeated, true),
        (Case::StoredNoise, "stored/noise", &noise, false),
        (Case::ZlibNoise, "zlib/noise", &noise, true),
    ] {
        if args.case.is_some_and(|selected| selected != case) {
            continue;
        }
        let encoded = wrapper(data, compressed);
        assert_eq!(
            decode_attachment(&encoded, &mut allowance(), &|| false)
                .unwrap()
                .data(),
            data
        );
        let start = Instant::now();
        for _ in 0..args.iterations {
            let decoded =
                decode_attachment(black_box(&encoded), &mut allowance(), &|| false).unwrap();
            black_box(decoded.data());
        }
        let seconds = start.elapsed().as_secs_f64();
        println!(
            "{name}\t{}\t{:.3}\t{:.1}",
            data.len(),
            seconds * MICROS_PER_SECOND / f64::from(args.iterations),
            data.len() as f64 * f64::from(args.iterations) / BYTES_PER_MIB / seconds
        );
    }
}
