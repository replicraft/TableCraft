//! Repeated public-API workloads for Hyperfine; no active database content.
use clap::{Parser, ValueEnum};
use std::{hint::black_box, io::Cursor, path::PathBuf};
use tablecraft_ace::{Header, inspect_catalog, read_table, read_table_index};
use tablecraft_core::{limits::ResourceBudget, values::LegacyDateTime};
use tablecraft_model::CatalogScope;

const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
const BENCH_BYTES: u64 = 256 * 1024 * 1024;
const BENCH_ROWS: u64 = 100_000;
const BENCH_STEPS: u64 = 10_000_000;

#[derive(Clone, Copy, ValueEnum)]
enum Workload {
    Header,
    Catalog,
    Sequential,
    Indexed,
    Calendar,
}

#[derive(Parser)]
struct Arguments {
    #[arg(value_enum)]
    workload: Workload,
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=10_000_000))]
    iterations: u32,
    #[arg(long)]
    database: Option<PathBuf>,
    #[arg(long, default_value = "ROULETTE_TB")]
    table: String,
    #[arg(long, default_value = "ORDR_NBR")]
    index: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let bytes = if matches!(args.workload, Workload::Calendar) {
        Vec::new()
    } else {
        let path = args.database.as_ref().ok_or("--database is required")?;
        if path.metadata()?.len() > MAX_INPUT_BYTES {
            return Err("benchmark input exceeds the native file limit".into());
        }
        std::fs::read(path)?
    };
    let mut checksum = 0_u64;
    for i in 0..args.iterations {
        let mut budget = ResourceBudget::new(BENCH_BYTES, BENCH_ROWS, BENCH_STEPS);
        let mut source = Cursor::new(black_box(bytes.as_slice()));
        let scope = CatalogScope::new(1);
        let result = match args.workload {
            Workload::Header => {
                u64::from(Header::parse(source.get_ref(), bytes.len() as u64)?.page_count())
            }
            Workload::Catalog => inspect_catalog(&mut source, scope, &mut budget, &|| false)?
                .catalog()
                .objects()
                .len() as u64,
            Workload::Sequential => {
                read_table(&mut source, scope, &args.table, &mut budget, &|| false)?
                    .rows()
                    .len() as u64
            }
            Workload::Indexed => read_table_index(
                &mut source,
                scope,
                &args.table,
                &args.index,
                &mut budget,
                &|| false,
            )?
            .rows()
            .len() as u64,
            Workload::Calendar => {
                // Exercise both signs, varying day/time components and nanosecond rounding.
                let days = f64::from(black_box(i % 1_000_000)) - 500_000.25;
                let value = LegacyDateTime::from_ole_days(days)?;
                let calendar = black_box(value).calendar_rounded_to_nanos()?;
                LegacyDateTime::from_calendar(black_box(calendar))?.to_bits()
            }
        };
        checksum = checksum.wrapping_add(black_box(result));
    }
    println!("{}", black_box(checksum));
    Ok(())
}
