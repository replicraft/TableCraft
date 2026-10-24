//! Public native metadata inspection and bounded collection contenders.
//! Container allocation counts are capacity/spill-derived, not global observations.
use arrayvec::ArrayVec;
use clap::{Parser, ValueEnum};
use smallvec::SmallVec;
use std::{hint::black_box, io::Cursor, path::PathBuf};
use tablecraft_ace::{IndexColumn, NativeIndex, inspect_table_indexes};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const INDEX_COLUMNS: usize = 10;
const INPUT_BYTES: u64 = 64 * 1024 * 1024;
const WORK_BYTES: u64 = 256 * 1024 * 1024;
const WORK_ROWS: u64 = 100_000;
const WORK_STEPS: u64 = 10_000_000;

#[derive(Clone, Copy, ValueEnum)]
enum Workload {
    Inspect,
    Vec,
    Arrayvec,
    Smallvec,
    Describe,
}

#[derive(Parser)]
struct Arguments {
    #[arg(value_enum)]
    workload: Workload,
    #[arg(long)]
    database: PathBuf,
    #[arg(long)]
    table: String,
    #[arg(long, default_value_t = 1_000, value_parser = clap::value_parser!(u32).range(1..=10_000_000))]
    iterations: u32,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(WORK_BYTES, WORK_ROWS, WORK_STEPS)
}

fn observe(columns: &[IndexColumn]) -> u64 {
    columns.iter().fold(0_u64, |sum, column| {
        sum.wrapping_mul(31)
            .wrapping_add(column.number() as u64)
            .wrapping_add(u64::from(column.descending()))
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = Arguments::parse();
    if arguments.database.metadata()?.len() > INPUT_BYTES {
        return Err("benchmark input exceeds its memory limit".into());
    }
    let bytes = std::fs::read(&arguments.database)?;
    let original = inspect_table_indexes(
        &mut Cursor::new(bytes.as_slice()),
        CatalogScope::new(1),
        &arguments.table,
        &mut budget(),
        &|| false,
    )?;
    if original.indexes().is_empty() {
        return Err("benchmark table needs an index".into());
    }

    if matches!(arguments.workload, Workload::Describe) {
        // Description output targets original ASCII fixture names only.
        // Reject names needing JSON escaping rather than inventing a codec.
        if original.indexes().iter().any(|index| {
            !index
                .name()
                .bytes()
                .all(|byte| byte.is_ascii_graphic() && !matches!(byte, b'"' | b'\\'))
        }) {
            return Err("benchmark description needs simple ASCII index names".into());
        }
        println!(
            "{{\"index_column_bytes\":{},\"native_index_bytes\":{},\"vec_bytes\":{},\"arrayvec_bytes\":{},\"smallvec_bytes\":{},\"indexes\":[{}]}}",
            size_of::<IndexColumn>(),
            size_of::<NativeIndex>(),
            size_of::<Vec<IndexColumn>>(),
            size_of::<ArrayVec<IndexColumn, INDEX_COLUMNS>>(),
            size_of::<SmallVec<[IndexColumn; 2]>>(),
            original
                .indexes()
                .iter()
                .map(|index| {
                    format!(
                        "{{\"name\":\"{}\",\"number\":{},\"data_number\":{},\"root\":{},\"usage\":{},\"kind\":{},\"flags\":{},\"columns\":[{}]}}",
                        index.name(),
                        index.number(),
                        index.data_number(),
                        index.root_page(),
                        index.usage_map(),
                        index.native_type(),
                        index.flags(),
                        index
                            .columns()
                            .iter()
                            .map(|column| {
                                format!("[{},{}]", column.number(), column.descending())
                            })
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        );
        return Ok(());
    }

    let mut checksum = 0_u64;
    let mut allocations = 0_u64;
    let mut requested_bytes = 0_u64;
    for _ in 0..arguments.iterations {
        match arguments.workload {
            Workload::Inspect => {
                let indexes = inspect_table_indexes(
                    &mut Cursor::new(black_box(bytes.as_slice())),
                    CatalogScope::new(1),
                    &arguments.table,
                    &mut budget(),
                    &|| false,
                )?;
                for index in indexes.indexes() {
                    checksum = checksum.wrapping_add(observe(black_box(index.columns())));
                }
            }
            Workload::Vec | Workload::Arrayvec | Workload::Smallvec => {
                for index in original.indexes() {
                    let input = black_box(index.columns());
                    let value = match arguments.workload {
                        Workload::Vec => {
                            let mut output = Vec::new();
                            output.try_reserve_exact(INDEX_COLUMNS)?;
                            output.extend_from_slice(input);
                            allocations += u64::from(output.capacity() != 0);
                            requested_bytes +=
                                (output.capacity() * size_of::<IndexColumn>()) as u64;
                            observe(black_box(output.as_slice()))
                        }
                        Workload::Arrayvec => {
                            let mut output = ArrayVec::<IndexColumn, INDEX_COLUMNS>::new();
                            for column in input {
                                output.try_push(*column)?;
                            }
                            observe(black_box(output.as_slice()))
                        }
                        Workload::Smallvec => {
                            let mut output = SmallVec::<[IndexColumn; 2]>::new();
                            output
                                .try_reserve_exact(input.len())
                                .map_err(|_| "small collection allocation failed")?;
                            output.extend_from_slice(input);
                            if output.spilled() {
                                allocations += 1;
                                requested_bytes +=
                                    (output.capacity() * size_of::<IndexColumn>()) as u64;
                            }
                            observe(black_box(output.as_slice()))
                        }
                        _ => unreachable!(),
                    };
                    checksum = checksum.wrapping_add(value);
                }
            }
            Workload::Describe => unreachable!(),
        }
    }
    println!(
        "{{\"checksum\":{},\"iterations\":{},\"container_heap_allocations\":{},\"container_heap_bytes\":{}}}",
        black_box(checksum),
        arguments.iterations,
        allocations,
        requested_bytes,
    );
    Ok(())
}
