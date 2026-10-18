//! Synthetic bitmap kernels compared independently of file I/O/query startup.
use clap::{Parser, ValueEnum};
use std::hint::black_box;

#[path = "../src/format/bitmap/kernel.rs"]
mod bitmap;
#[path = "support/bitmap_simd.rs"]
mod simd;

const INPUT_BYTES: usize = 4096;
const SPARSE_STRIDE: usize = 97;
const BITS_PER_BYTE: usize = u8::BITS as usize;

#[derive(Clone, Copy, ValueEnum)]
enum Kernel {
    Scalar,
    Words,
    Simd,
}

#[derive(Clone, Copy, ValueEnum)]
enum Density {
    Zero,
    Sparse,
    Mixed,
    Dense,
}

#[derive(Parser)]
struct Arguments {
    #[arg(value_enum)]
    kernel: Kernel,
    #[arg(long, value_enum, default_value = "sparse")]
    density: Density,
    #[arg(long, default_value_t = 10_000, value_parser = clap::value_parser!(u32).range(1..=10_000_000))]
    iterations: u32,
}

fn main() {
    let args = Arguments::parse();
    let input: Vec<u8> = (0..INPUT_BYTES)
        .map(|i| match args.density {
            Density::Zero => 0,
            Density::Sparse => {
                if i % SPARSE_STRIDE == 0 {
                    0x81
                } else {
                    0
                }
            }
            Density::Mixed => i.wrapping_mul(37) as u8,
            Density::Dense => u8::MAX,
        })
        .collect();
    let mut checksum = 0_u64;
    for _ in 0..args.iterations {
        let bytes = black_box(input.as_slice());
        let result: u64 = match args.kernel {
            Kernel::Scalar => bytes
                .iter()
                .enumerate()
                .map(|(i, byte)| {
                    (0..BITS_PER_BYTE)
                        .filter(|bit| byte & (1 << bit) != 0)
                        .map(|bit| (i * BITS_PER_BYTE + bit) as u64)
                        .sum::<u64>()
                })
                .sum(),
            Kernel::Words => {
                let mut sum = 0;
                for (block, bytes) in bytes.chunks(bitmap::BLOCK_BYTES).enumerate() {
                    for bit in bitmap::set_bits(bytes) {
                        sum += (block * bitmap::BLOCK_BYTES * BITS_PER_BYTE + bit) as u64;
                    }
                }
                sum
            }
            Kernel::Simd => bytes
                .chunks(simd::BLOCK_BYTES)
                .enumerate()
                .map(|(block, bytes)| {
                    simd::set_bits(bytes)
                        .map(|bit| (block * simd::BLOCK_BYTES * BITS_PER_BYTE + bit) as u64)
                        .sum::<u64>()
                })
                .sum(),
        };
        checksum = checksum.wrapping_add(black_box(result));
    }
    println!("{}", black_box(checksum));
}
