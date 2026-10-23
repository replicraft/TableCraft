//! Engineering-only exact original attachment bytes for the complex-row recipe.
use clap::Parser;
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use tablecraft_ace::{AttachmentDecoder, TableSnapshotLimits, read_attachment_column_with_limits};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const BYTES: u64 = 16 * 1024 * 1024;
const DEFAULT_ROWS: u32 = 16_384;
const STEPS: u64 = 1_000_000;
const MAX_STEPS: u64 = 100_000_000;
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const ROW_FORMAT_BYTES: usize = 64;

#[derive(Parser)]
struct Arguments {
    database: PathBuf,
    #[arg(long, default_value = "Collections")]
    table: String,
    #[arg(long, default_value = "Files")]
    column: String,
    /// Emit exact stored metadata and encoded data instead of decoded file bytes.
    #[arg(long)]
    metadata: bool,
    /// Explicit native/decoder allocation and read allowance.
    #[arg(long, default_value_t = BYTES, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    bytes: u64,
    /// Cumulative native rows and complete parent/child snapshot ceiling.
    #[arg(long, default_value_t = DEFAULT_ROWS, value_parser = clap::value_parser!(u32).range(1..=i64::from(TableSnapshotLimits::MAX_ROWS)))]
    rows: u32,
    /// Explicit native/decoder work allowance.
    #[arg(long, default_value_t = STEPS, value_parser = clap::value_parser!(u64).range(1..=MAX_STEPS))]
    steps: u64,
}

fn text(output: &mut impl Write, value: Option<&str>) -> io::Result<()> {
    match value {
        None => write!(output, "N"),
        Some(value) => {
            write!(output, "T")?;
            for byte in value.as_bytes() {
                write!(output, "{byte:02x}")?;
            }
            Ok(())
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let mut budget = ResourceBudget::new(args.bytes, u64::from(args.rows), args.steps);
    let collection = read_attachment_column_with_limits(
        &mut File::open(args.database)?,
        CatalogScope::new(1),
        &args.table,
        &args.column,
        TableSnapshotLimits::new(args.rows, TableSnapshotLimits::default().batch_rows())?,
        &mut budget,
        &|| false,
    )?;
    let cancelled = || false;
    let mut decoder = AttachmentDecoder::new(&mut budget, &cancelled);
    let mut output = Vec::new();
    for parent in 0..collection.links().parents().len() {
        for attachment in collection
            .attachments(parent)
            .ok_or("invalid parent ordinal")?
        {
            let attachment = attachment?;
            let id = attachment.identity();
            if args.metadata {
                let allowance = 2
                    * (attachment.file_url().map_or(0, str::len)
                        + attachment.file_name().map_or(0, str::len)
                        + attachment.file_type().map_or(0, str::len)
                        + attachment.encoded_file_data().map_or(0, <[u8]>::len))
                    + ROW_FORMAT_BYTES * 2;
                if output.len() + allowance > OUTPUT_BYTES {
                    return Err("engineering output limit exceeded".into());
                }
                decoder
                    .budget_mut()
                    .charge_allocation(allowance as u64, 1)?;
                output.try_reserve(allowance)?;
                write!(output, "{parent}\t{id}\t")?;
                text(&mut output, attachment.file_url())?;
                write!(output, "\t")?;
                text(&mut output, attachment.file_name())?;
                write!(output, "\t")?;
                text(&mut output, attachment.file_type())?;
                write!(output, "\t")?;
                match attachment.encoded_file_data() {
                    None => write!(output, "N")?,
                    Some(bytes) => {
                        write!(output, "X")?;
                        for byte in bytes {
                            write!(output, "{byte:02x}")?;
                        }
                    }
                }
                write!(output, "\t")?;
                match attachment.file_timestamp() {
                    None => write!(output, "N")?,
                    Some(value) => write!(output, "Y64:{:016x}", value.to_bits())?,
                }
                write!(output, "\t")?;
                match attachment.file_flags() {
                    None => write!(output, "N")?,
                    Some(value) => write!(output, "I32:{value}")?,
                }
                writeln!(output)?;
                continue;
            }
            let decoded = attachment
                .decode_with(&mut decoder)?
                .ok_or("example requires a present attachment payload")?;
            let allowance =
                2 * (decoded.type_label().len() + decoded.data().len()) + ROW_FORMAT_BYTES;
            if output.len() + allowance > OUTPUT_BYTES {
                return Err("engineering output limit exceeded".into());
            }
            decoder
                .budget_mut()
                .charge_allocation(allowance as u64, 1)?;
            output.try_reserve(allowance)?;
            write!(output, "{parent}\t{id}\t")?;
            for byte in decoded.type_label().as_bytes() {
                write!(output, "{byte:02x}")?;
            }
            write!(output, "\t")?;
            for byte in decoded.data() {
                write!(output, "{byte:02x}")?;
            }
            writeln!(output)?;
        }
    }
    io::stdout().lock().write_all(&output)?;
    Ok(())
}
