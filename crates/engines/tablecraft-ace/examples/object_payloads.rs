//! Explicit inert opaque-object bytes for independent comparison, never execution.
use clap::Parser;
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use tablecraft_ace::{InspectError, NativeImage, OpaquePayload, PayloadPresence};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ROWS: u64 = 100_000;
const MAX_STEPS: u64 = 100_000_000;

#[derive(Parser)]
struct Arguments {
    database: PathBuf,
    #[arg(allow_negative_numbers = true)]
    native_id: i32,
}
fn write_payload(
    output: &mut impl Write,
    payload: &OpaquePayload,
) -> Result<(), Box<dyn std::error::Error>> {
    match payload.presence() {
        PayloadPresence::Absent => write!(output, "N")?,
        PayloadPresence::Uninspected => write!(output, "U")?,
        PayloadPresence::Present => {
            let bytes = payload.as_bytes().ok_or(InspectError::Malformed)?;
            write!(output, "X")?;
            for byte in bytes {
                write!(output, "{byte:02x}")?;
            }
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let mut budget = ResourceBudget::new(MAX_BYTES, MAX_ROWS, MAX_STEPS);
    let image = NativeImage::capture(
        &mut File::open(args.database)?,
        CatalogScope::new(1),
        &mut budget,
        &|| false,
    )?;
    let object = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.native_id() == args.native_id)
        .ok_or(InspectError::UnknownObject)?
        .object_id();
    let payloads = image.read_object_payloads(object, &mut budget, &|| false)?;
    let mut output = io::BufWriter::new(io::stdout().lock());
    for (name, payload) in [
        ("Lv", payloads.body()),
        ("LvModule", payloads.module()),
        ("LvExtra", payloads.extra()),
    ] {
        write!(output, "{name}\t")?;
        write_payload(&mut output, payload)?;
        writeln!(output)?;
    }
    output.flush()?;
    Ok(())
}
