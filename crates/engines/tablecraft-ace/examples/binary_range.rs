//! Engineering-only bounded Binary index selections, with atomic tagged output.
use clap::{Parser, ValueEnum};
use std::{
    fs::File,
    io::{self, Write},
    ops::Bound,
    path::PathBuf,
};
use tablecraft_ace::{
    BinaryIndexSelection, IndexProjection, IndexProjectionSelection, read_table_binary_index_range,
    read_table_index_projection,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const BYTES: u64 = 32 * 1024 * 1024;
const ROWS: u64 = 16_384;
const STEPS: u64 = 1_000_000;
const OUTPUT_BYTES: usize = 1024 * 1024;
#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    All,
    Null,
    Range,
}
#[derive(Parser)]
struct Arguments {
    database: PathBuf,
    #[arg(long, default_value = "BinaryIndexed")]
    table: String,
    #[arg(long, default_value = "BinaryAsc")]
    index: String,
    #[arg(long, value_enum, default_value = "range")]
    mode: Mode,
    #[arg(long)]
    lower: Option<String>,
    #[arg(long)]
    upper: Option<String>,
    #[arg(long, requires = "lower")]
    exclude_lower: bool,
    #[arg(long, requires = "upper")]
    exclude_upper: bool,
    #[arg(long)]
    project_id: bool,
}
fn hex(text: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if text.len() > 2 * BinaryIndexSelection::MAX_BOUND_BYTES
        || !text.len().is_multiple_of(2)
        || !text.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("invalid bounded hexadecimal endpoint".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|position| u8::from_str_radix(&text[position..position + 2], 16).map_err(Into::into))
        .collect()
}
fn bound(value: Option<&[u8]>, excluded: bool) -> Bound<&[u8]> {
    match value {
        None => Bound::Unbounded,
        Some(value) if excluded => Bound::Excluded(value),
        Some(value) => Bound::Included(value),
    }
}
struct Output(Vec<u8>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > OUTPUT_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other("binary range output limit exceeded"));
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(|_| io::Error::other("binary range output allocation failed"))?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let lower = args.lower.as_deref().map(hex).transpose()?;
    let upper = args.upper.as_deref().map(hex).transpose()?;
    if !matches!(args.mode, Mode::Range) && (lower.is_some() || upper.is_some()) {
        return Err("bounds require range mode".into());
    }
    let selection = match args.mode {
        Mode::All => BinaryIndexSelection::All,
        Mode::Null => BinaryIndexSelection::Null,
        Mode::Range => BinaryIndexSelection::Range {
            lower: bound(lower.as_deref(), args.exclude_lower),
            upper: bound(upper.as_deref(), args.exclude_upper),
        },
    };
    let mut source = File::open(args.database)?;
    let mut budget = ResourceBudget::new(BYTES, ROWS, STEPS);
    let table = if args.project_id {
        read_table_index_projection(
            &mut source,
            CatalogScope::new(1),
            &args.table,
            &args.index,
            IndexProjection {
                columns: &["ID"],
                selection: IndexProjectionSelection::Binary(selection),
            },
            &mut budget,
            &|| false,
        )?
    } else {
        read_table_binary_index_range(
            &mut source,
            CatalogScope::new(1),
            &args.table,
            &args.index,
            selection,
            &mut budget,
            &|| false,
        )?
    };
    let mut output = Output(Vec::new());
    for row in table.rows() {
        for (position, value) in row.iter().enumerate() {
            if position != 0 {
                write!(output, "\t")?;
            }
            match value {
                Value::Int32(value) => write!(output, "I32:{value}")?,
                Value::Null => write!(output, "N")?,
                Value::Binary(value) => {
                    write!(output, "X")?;
                    for byte in value {
                        write!(output, "{byte:02x}")?;
                    }
                }
                _ => return Err("engineering range expects integer/binary fields only".into()),
            }
        }
        writeln!(output)?;
    }
    io::stdout().lock().write_all(&output.0)?;
    Ok(())
}
