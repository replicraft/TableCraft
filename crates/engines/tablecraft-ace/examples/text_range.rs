//! Engineering-only physical Text index intervals, with atomic tagged output.
//! Start/end follow index direction; these are not SQL comparison bounds.
use clap::{Parser, ValueEnum};
use std::{
    fs::File,
    io::{self, Write},
    ops::Bound,
    path::PathBuf,
};
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, TextIndexSelection, read_table_index_projection,
    read_table_text_index_range,
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
    #[arg(long, default_value = "TextIndexed")]
    table: String,
    #[arg(long, default_value = "TextAsc")]
    index: String,
    #[arg(long, value_enum, default_value = "range")]
    mode: Mode,
    /// First physical endpoint; descending intervals usually start with larger text.
    #[arg(long)]
    start: Option<String>,
    #[arg(long)]
    end: Option<String>,
    #[arg(long, requires = "start")]
    exclude_start: bool,
    #[arg(long, requires = "end")]
    exclude_end: bool,
    #[arg(long)]
    project_id: bool,
}
fn bound(value: Option<&str>, excluded: bool) -> Bound<&str> {
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
            return Err(io::Error::other("text range output limit exceeded"));
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(|_| io::Error::other("text range output allocation failed"))?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let start = args.start.as_deref();
    let end = args.end.as_deref();
    if !matches!(args.mode, Mode::Range) && (start.is_some() || end.is_some()) {
        return Err("bounds require range mode".into());
    }
    let selection = match args.mode {
        Mode::All => TextIndexSelection::All,
        Mode::Null => TextIndexSelection::Null,
        Mode::Range => TextIndexSelection::Range {
            start: bound(start, args.exclude_start),
            end: bound(end, args.exclude_end),
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
                selection: IndexProjectionSelection::Text(selection),
            },
            &mut budget,
            &|| false,
        )?
    } else {
        read_table_text_index_range(
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
                Value::Text(value) => {
                    write!(output, "T")?;
                    for byte in value.as_bytes() {
                        write!(output, "{byte:02x}")?;
                    }
                }
                _ => return Err("engineering range expects integer/text fields only".into()),
            }
        }
        writeln!(output)?;
    }
    io::stdout().lock().write_all(&output.0)?;
    Ok(())
}
