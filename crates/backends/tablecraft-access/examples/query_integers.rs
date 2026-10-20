//! Engineering-only integer query results under explicit native byte controls.
use clap::Parser;
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use tablecraft_access::NativeSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

const DEFAULT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const NATIVE_ROWS: u64 = 16_384;
const NATIVE_STEPS: u64 = 1_000_000;
const OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Parser)]
struct Arguments {
    database: PathBuf,
    sql: String,
    #[arg(long, default_value_t = DEFAULT_BYTES, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    bytes: u64,
}
struct Output(Vec<u8>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > OUTPUT_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other("integer query output limit exceeded"));
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(|_| io::Error::other("integer query output allocation failed"))?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let source = NativeSource::new(
        File::open(args.database)?,
        CatalogScope::new(1),
        ResourceBudget::new(args.bytes, NATIVE_ROWS, NATIVE_STEPS),
        &|| false,
    )?;
    let query = parse_query(&args.sql, ParseLimits::default())?;
    let result =
        prepare(query.query(), &source)?.execute(&source, &[], ExecutionOptions::default())?;
    let mut output = Output(Vec::new());
    for row in result.rows() {
        for (position, value) in row.iter().enumerate() {
            if position != 0 {
                write!(output, "\t")?;
            }
            match value {
                Value::Int32(value) => write!(output, "I32:{value}")?,
                Value::Int64(value) => write!(output, "I64:{value}")?,
                _ => return Err("engineering query must return Int32/Int64 only".into()),
            }
        }
        writeln!(output)?;
    }
    io::stdout().lock().write_all(&output.0)?;
    Ok(())
}
