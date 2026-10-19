//! Engineering-only tagged rows for comparison with TableDataOracle.java.
//! Stable explicit inputs, no active content; this is not the CLI output protocol.
use clap::Parser;
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use tablecraft_ace::{read_stored_calculation, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const DEFAULT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ROWS: u64 = 16_384;
const MAX_STEPS: u64 = 1_000_000;

#[derive(Parser)]
struct Arguments {
    database: PathBuf,
    #[arg(required = true, num_args = 1..=32)]
    tables: Vec<String>,
    /// Native allocation/read allowance per table; format ceilings still apply.
    #[arg(long, default_value_t = DEFAULT_BYTES, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    bytes: u64,
    /// Explicitly inspect this stored calculation instead of ordinary scalar rows.
    #[arg(long, value_name = "COLUMN")]
    stored_calculation: Option<String>,
    /// Emit exact legacy DateTime serial bits instead of a calendar rendering.
    #[arg(long)]
    date_bits: bool,
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
    let mut input = File::open(args.database)?;
    let mut output = io::BufWriter::new(io::stdout().lock());
    for name in args.tables {
        if let Some(column) = &args.stored_calculation {
            let stored = read_stored_calculation(
                &mut input,
                CatalogScope::new(1),
                &name,
                column,
                &mut ResourceBudget::new(args.bytes, MAX_ROWS, MAX_STEPS),
                &|| false,
            )?;
            for cell in stored.values() {
                write!(output, "{name}\t")?;
                value(&mut output, cell, args.date_bits)?;
                writeln!(output)?;
            }
            continue;
        }
        let table = read_table(
            &mut input,
            CatalogScope::new(1),
            &name,
            &mut ResourceBudget::new(args.bytes, MAX_ROWS, MAX_STEPS),
            &|| false,
        )?;
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
