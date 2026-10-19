//! Whole source/preparation/execution measurements; no application content runs.
use clap::{Parser, ValueEnum};
use std::{
    hint::black_box,
    io::{Cursor, Read},
    path::PathBuf,
};
use tablecraft_access::NativeSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};

const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
const NATIVE_BYTES: u64 = 16 * 1024 * 1024;
const NATIVE_ROWS: u64 = 16_384;
const NATIVE_STEPS: u64 = 1_000_000;

#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    Full,
    Seek,
}
#[derive(Clone, Copy, ValueEnum)]
enum Workload {
    Range,
    Equality,
    Null,
}
#[derive(Parser)]
struct Arguments {
    #[arg(value_enum)]
    mode: Mode,
    #[arg(long)]
    database: PathBuf,
    #[arg(long, value_enum)]
    workload: Workload,
    #[arg(long, default_value_t = 500, value_parser = clap::value_parser!(u64).range(1..=1_000_000))]
    iterations: u64,
}
// Same current schema/binding/evaluation path, with only the optional source
// constraint declined. This avoids comparing unrelated code or toolchains.
struct FullScan<'a>(&'a dyn Source);
impl Source for FullScan<'_> {
    fn columns(&self, table: &str) -> Result<&[tablecraft_schema::Column], QueryError> {
        self.0.columns(table)
    }
    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.0.scan(table, visitor)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let mut bytes = Vec::new();
    std::fs::File::open(args.database)?
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err("benchmark input exceeds limit".into());
    }
    let (index, filter) = match args.workload {
        Workload::Range => ("PrimaryKey", "ID >= 0 AND ID <= 9"),
        Workload::Equality => ("CategoryDesc", "Category = 0"),
        Workload::Null => ("CategoryDesc", "Category IS NULL"),
    };
    let parsed = tablecraft_sql::parse_query(
        &format!("SELECT ID, Category FROM Indexed WHERE {filter} ORDER BY ID"),
        Default::default(),
    )?;
    let (mut count, mut ids, mut nulls) = (0_u64, 0_i64, 0_u64);
    for _ in 0..args.iterations {
        let native = NativeSource::new(
            Cursor::new(bytes.as_slice()),
            CatalogScope::new(1),
            ResourceBudget::new(NATIVE_BYTES, NATIVE_ROWS, NATIVE_STEPS),
            &|| false,
        )?
        .with_index("Indexed", index)?;
        let full = FullScan(&native);
        let source: &dyn Source = match args.mode {
            Mode::Full => &full,
            Mode::Seek => &native,
        };
        let result = black_box(prepare(parsed.query(), source)?.execute(
            source,
            &[],
            ExecutionOptions::default(),
        )?);
        count += result.rows().len() as u64;
        for row in result.rows() {
            let Value::Int32(id) = row[0] else {
                return Err("benchmark requires Int32 ID".into());
            };
            ids += i64::from(id);
            nulls += u64::from(matches!(row[1], Value::Null));
        }
    }
    println!(
        "{} {} {}",
        black_box(count),
        black_box(ids),
        black_box(nulls)
    );
    Ok(())
}
