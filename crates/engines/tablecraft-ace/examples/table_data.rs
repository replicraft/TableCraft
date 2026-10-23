//! Engineering-only tagged rows for comparison with TableDataOracle.java.
//! Stable explicit inputs, no active content; this is not the CLI output protocol.
use clap::Parser;
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use tablecraft_ace::{
    TableCursor, TablePageRequest, TableSnapshotLimits, read_multivalue_column_with_limits,
    read_stored_calculation, read_table, read_table_index, read_version_history_column_with_limits,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const DEFAULT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const DEFAULT_ROWS: u32 = 16_384;
const DEFAULT_STEPS: u64 = 1_000_000;
const MAX_STEPS: u64 = 100_000_000;
const MAX_PROJECTED_COLUMNS: usize = 256;

#[derive(Parser)]
struct Arguments {
    database: PathBuf,
    #[arg(required = true, num_args = 1..=32)]
    tables: Vec<String>,
    /// Native allocation/read allowance per table; format ceilings still apply.
    #[arg(long, default_value_t = DEFAULT_BYTES, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    bytes: u64,
    /// Explicit work allowance per table, including long-value decoding.
    #[arg(long, default_value_t = DEFAULT_STEPS, value_parser = clap::value_parser!(u64).range(1..=MAX_STEPS))]
    steps: u64,
    /// Explicitly inspect this stored calculation instead of ordinary scalar rows.
    #[arg(long, value_name = "COLUMN", conflicts_with = "index")]
    stored_calculation: Option<String>,
    /// Read each named table through this required native index (no fallback).
    #[arg(long, value_name = "INDEX")]
    index: Option<String>,
    /// Emit exact legacy DateTime serial bits instead of a calendar rendering.
    #[arg(long)]
    date_bits: bool,
    /// Resume scalar batches with explicit fresh per-batch controls (engineering only).
    #[arg(long, conflicts_with_all = ["index", "stored_calculation", "multivalues", "version_history"], value_parser = clap::value_parser!(u32).range(1..=i64::from(TablePageRequest::MAX_ROWS)))]
    batch_size: Option<u32>,
    /// Explicit projected cursor fields (repeatable; emitted in native order).
    #[arg(long = "column", requires = "batch_size")]
    columns: Vec<String>,
    /// Explicit cumulative collection rows and parent/child snapshot ceiling (default 16,384).
    #[arg(long, value_name = "ROWS", value_parser = clap::value_parser!(u32).range(1..=i64::from(TableSnapshotLimits::MAX_ROWS)))]
    collection_rows: Option<u32>,
    /// Explicitly inspect stored multivalue collections instead of scalar rows.
    #[arg(long, value_name = "COLUMN", conflicts_with_all = ["index", "stored_calculation"])]
    multivalues: Option<String>,
    /// Inspect stored history for this visible Memo, resolving its hidden column.
    #[arg(long, value_name = "MEMO", conflicts_with_all = ["index", "stored_calculation", "multivalues"])]
    version_history: Option<String>,
}

fn hex(out: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    for byte in bytes {
        write!(out, "{byte:02x}")?;
    }
    Ok(())
}

fn value(
    out: &mut impl Write,
    value: &Value,
    date_bits: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match value {
        Value::Null => write!(out, "N")?,
        Value::Boolean(v) => write!(out, "B{}", u8::from(*v))?,
        Value::Byte(v) => write!(out, "U{v}")?,
        Value::Int16(v) => write!(out, "I16:{v}")?,
        Value::Int32(v) => write!(out, "I32:{v}")?,
        Value::Int64(v) => write!(out, "I64:{v}")?,
        Value::Currency(v) => write!(out, "C{}", v.scaled())?,
        Value::Decimal(v) => write!(out, "Q{}:{}", v.coefficient(), v.scale())?,
        Value::Single(v) => write!(out, "F{:08x}", v.to_bits())?,
        Value::Double(v) => write!(out, "D{:016x}", v.to_bits())?,
        Value::Text(v) => {
            write!(out, "T")?;
            hex(out, v.as_bytes())?;
        }
        Value::Binary(v) => {
            write!(out, "X")?;
            hex(out, v)?;
        }
        Value::Guid(v) => {
            write!(out, "G")?;
            hex(out, &v.to_bytes())?;
        }
        Value::DateTime(v) => {
            if date_bits {
                write!(out, "Y64:{:016x}", v.to_bits())?;
                return Ok(());
            }
            let d = v.calendar_rounded_to_nanos()?;
            write!(
                out,
                "Y{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:09}",
                d.year(),
                d.month(),
                d.day(),
                d.hour(),
                d.minute(),
                d.second(),
                d.subsec_nanosecond()
            )?;
        }
        Value::DateTimeExtended(value) => {
            let (year, month, day) = value.date();
            let (hour, minute, second, fraction) = value.time();
            write!(
                out,
                "Z{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{fraction:07}"
            )?;
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    if args.collection_rows.is_some()
        && args.version_history.is_none()
        && args.multivalues.is_none()
    {
        return Err("collection rows require --version-history or --multivalues".into());
    }
    let collection_rows = args.collection_rows.unwrap_or(DEFAULT_ROWS);
    let collection_limits =
        TableSnapshotLimits::new(collection_rows, TableSnapshotLimits::default().batch_rows())?;
    let mut input = File::open(args.database)?;
    let mut output = io::BufWriter::new(io::stdout().lock());
    for name in args.tables {
        if let Some(limit) = args.batch_size {
            let mut opening = ResourceBudget::new(args.bytes, u64::from(DEFAULT_ROWS), args.steps);
            let mut cursor = if args.columns.is_empty() {
                TableCursor::open(
                    &mut input,
                    CatalogScope::new(1),
                    &name,
                    &mut opening,
                    &|| false,
                )?
            } else {
                if args.columns.len() > MAX_PROJECTED_COLUMNS {
                    return Err("too many projected columns".into());
                }
                let selected = args.columns.iter().map(String::as_str).collect::<Vec<_>>();
                TableCursor::open_projection(
                    &mut input,
                    CatalogScope::new(1),
                    &name,
                    &selected,
                    &mut opening,
                    &|| false,
                )?
            };
            loop {
                let batch = cursor.next_page(
                    limit,
                    &mut ResourceBudget::new(args.bytes, u64::from(DEFAULT_ROWS), args.steps),
                    &|| false,
                )?;
                for row in batch.rows() {
                    write!(output, "{name}")?;
                    for cell in row {
                        write!(output, "\t")?;
                        value(&mut output, cell, args.date_bits)?;
                    }
                    writeln!(output)?;
                }
                if batch.reached_end() {
                    break;
                }
            }
            continue;
        }
        if let Some(memo) = &args.version_history {
            let history = read_version_history_column_with_limits(
                &mut input,
                CatalogScope::new(1),
                &name,
                memo,
                collection_limits,
                &mut ResourceBudget::new(args.bytes, u64::from(collection_rows), args.steps),
                &|| false,
            )?;
            for (ordinal, parent) in history.links().parents().iter().enumerate() {
                write!(output, "{name}\t{ordinal}\t")?;
                match parent.key() {
                    Some(key) => write!(output, "{key}")?,
                    None => write!(output, "N")?,
                }
                for version in history
                    .stored_versions(ordinal)
                    .ok_or("missing validated parent")?
                {
                    let version = version?;
                    write!(output, "\tI32:{}\t", version.identity())?;
                    match version.value() {
                        None => write!(output, "N")?,
                        Some(value) => {
                            write!(output, "T")?;
                            hex(&mut output, value.as_bytes())?;
                        }
                    }
                    write!(output, "\t")?;
                    match version.modified() {
                        None => write!(output, "N")?,
                        Some(value) => write!(output, "Y64:{:016x}", value.to_bits())?,
                    }
                }
                writeln!(output)?;
            }
            continue;
        }
        if let Some(column) = &args.multivalues {
            let collections = read_multivalue_column_with_limits(
                &mut input,
                CatalogScope::new(1),
                &name,
                column,
                collection_limits,
                &mut ResourceBudget::new(args.bytes, u64::from(collection_rows), args.steps),
                &|| false,
            )?;
            for (ordinal, parent) in collections.links().parents().iter().enumerate() {
                write!(output, "{name}\t{ordinal}\t")?;
                match parent.key() {
                    Some(key) => write!(output, "{key}")?,
                    None => write!(output, "N")?,
                }
                for cell in collections
                    .values(ordinal)
                    .ok_or("missing validated parent")?
                {
                    write!(output, "\t")?;
                    value(&mut output, cell, args.date_bits)?;
                }
                writeln!(output)?;
            }
            continue;
        }
        if let Some(column) = &args.stored_calculation {
            let stored = read_stored_calculation(
                &mut input,
                CatalogScope::new(1),
                &name,
                column,
                &mut ResourceBudget::new(args.bytes, u64::from(DEFAULT_ROWS), args.steps),
                &|| false,
            )?;
            for cell in stored.values() {
                write!(output, "{name}\t")?;
                value(&mut output, cell, args.date_bits)?;
                writeln!(output)?;
            }
            continue;
        }
        let table = if let Some(index) = &args.index {
            read_table_index(
                &mut input,
                CatalogScope::new(1),
                &name,
                index,
                &mut ResourceBudget::new(args.bytes, u64::from(DEFAULT_ROWS), args.steps),
                &|| false,
            )?
        } else {
            read_table(
                &mut input,
                CatalogScope::new(1),
                &name,
                &mut ResourceBudget::new(args.bytes, u64::from(DEFAULT_ROWS), args.steps),
                &|| false,
            )?
        };
        for row in table.rows() {
            write!(output, "{name}")?;
            for cell in row {
                write!(output, "\t")?;
                value(&mut output, cell, args.date_bits)?;
            }
            writeln!(output)?;
        }
    }
    output.flush()?;
    Ok(())
}
