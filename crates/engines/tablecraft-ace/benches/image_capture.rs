//! Public-API snapshot capture comparison for Hyperfine and platform RSS tools.
use clap::{Parser, ValueEnum};
use std::{hint::black_box, io::Cursor, path::PathBuf};
use tablecraft_ace::{NativeImage, NativeImageSource};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const INPUT_LIMIT: u64 = 64 * 1024 * 1024;
const WORK_BYTES: u64 = 512 * 1024 * 1024;
const WORK_ROWS: u64 = 1_000_000;
const WORK_STEPS: u64 = 100_000_000;

#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    Stream,
    Shared,
}

#[derive(Parser)]
struct Arguments {
    #[arg(value_enum)]
    mode: Mode,
    #[arg(long)]
    database: PathBuf,
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=1_000_000))]
    iterations: u32,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(WORK_BYTES, WORK_ROWS, WORK_STEPS)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();

    if args.database.metadata()?.len() > INPUT_LIMIT {
        return Err("benchmark source exceeds complete-image envelope".into());
    }

    let bytes = std::fs::read(&args.database)?;
    let source = NativeImageSource::from_vec(bytes, &mut budget(), &|| false)?;
    let mut checksum = 0;

    for _ in 0..args.iterations {
        let mut work = budget();
        let image = match args.mode {
            Mode::Stream => NativeImage::capture(
                &mut Cursor::new(source.as_bytes()),
                CatalogScope::new(1),
                &mut work,
                &|| false,
            )?,
            Mode::Shared => NativeImage::capture_shared(
                source.clone(),
                CatalogScope::new(1),
                &mut work,
                &|| false,
            )?,
        };

        assert_eq!(image.as_bytes(), source.as_bytes());
        if matches!(args.mode, Mode::Shared) {
            assert_eq!(image.as_bytes().as_ptr(), source.as_bytes().as_ptr());
        }

        checksum ^= black_box(image.catalog().header().page_count());
    }

    let copied_payload_bytes = match args.mode {
        Mode::Stream => source.as_bytes().len() as u64 * u64::from(args.iterations),
        Mode::Shared => 0,
    };
    // These are API-path counts, not allocator instrumentation or total RSS.
    // The stream allocator may reserve more than the requested payload length.
    let original_retained_capacity = source.retained_capacity();
    let additional_payload_bytes_requested = if matches!(args.mode, Mode::Stream) {
        source.as_bytes().len()
    } else {
        0
    };

    println!(
        "checksum={checksum} copied_payload_bytes={copied_payload_bytes} original_retained_capacity={original_retained_capacity} additional_payload_bytes_requested={additional_payload_bytes_requested}"
    );

    Ok(())
}
