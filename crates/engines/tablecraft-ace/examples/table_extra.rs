//! Bounded inert LvExtra comparison output; no XML interpretation or execution.
use clap::Parser;
use std::{
    error::Error,
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use tablecraft_ace::{InspectError, NativeImage, TableExtra};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::{CatalogScope, ObjectKind};

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ROWS: u64 = 100_000;
const MAX_STEPS: u64 = 100_000_000;
const MAX_OUTPUT_BYTES: u64 = 128 * 1024 * 1024;
const HEX: &[u8; 16] = b"0123456789abcdef";

#[derive(Parser)]
#[command(about = "Inspect one exact local table's inert LvExtra as bounded hex TSV")]
struct Arguments {
    database: PathBuf,
    /// Exact catalog table name. Ambiguous or differently cased names refuse.
    table: String,
    #[arg(long, default_value_t = MAX_BYTES, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    bytes: u64,
    #[arg(long, default_value_t = MAX_ROWS, value_parser = clap::value_parser!(u64).range(1..=MAX_ROWS))]
    rows: u64,
    #[arg(long, default_value_t = MAX_STEPS, value_parser = clap::value_parser!(u64).range(1..=MAX_STEPS))]
    steps: u64,
    #[arg(long, default_value_t = MAX_OUTPUT_BYTES, value_parser = clap::value_parser!(u64).range(1..=MAX_OUTPUT_BYTES))]
    output_bytes: u64,
}

// Reserve once from an independently checked upper bound before formatting.
// The sink refuses any underestimated output instead of allocating implicitly.
struct Output {
    bytes: Vec<u8>,
    capacity: usize,
    work: ResourceBudget,
}

impl Output {
    fn new(capacity: usize, limit: u64) -> Result<Self, Box<dyn Error>> {
        let mut work = ResourceBudget::new(limit, 0, MAX_STEPS);
        work.charge_allocation(capacity as u64, 1)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(capacity)?;
        Ok(Self {
            bytes,
            capacity,
            work,
        })
    }

    fn hex(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut encoded = [0; 256];
        for chunk in bytes.chunks(encoded.len() / 2) {
            for (pair, byte) in encoded.as_chunks_mut::<2>().0.iter_mut().zip(chunk) {
                pair[0] = HEX[usize::from(byte >> 4)];
                pair[1] = HEX[usize::from(byte & 15)];
            }
            self.write_all(&encoded[..chunk.len() * 2])?;
        }
        Ok(())
    }
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let end = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|end| *end <= self.capacity)
            .ok_or_else(|| io::Error::other("table Extra output bound exceeded"))?;
        self.work
            .charge(Resource::Steps, bytes.len() as u64)
            .map_err(io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        debug_assert_eq!(self.bytes.len(), end);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn output_bound(
    extra: &TableExtra,
    table: &str,
    budget: &mut ResourceBudget,
) -> Result<usize, InspectError> {
    let mut bound = 0usize;
    let mut add = |bytes: usize, framing: usize| -> Result<(), InspectError> {
        bound = bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(framing))
            .and_then(|bytes| bound.checked_add(bytes))
            .ok_or(InspectError::StructureLimit)?;
        Ok(())
    };
    add(table.len(), 512)?;
    add(extra.raw().map_or(0, <[u8]>::len), 128)?;
    for name in extra.names() {
        budget.charge(Resource::Steps, 1)?;
        add(name.len(), 32)?;
    }
    for group in extra.groups() {
        budget.charge(Resource::Steps, 1)?;
        add(group.raw().len(), 64)?;
        for property in group.properties().unwrap_or(&[]) {
            budget.charge(Resource::Steps, 1)?;
            add(property.name().len(), 96)?;
            add(property.raw().len(), 0)?;
            add(property.text().map_or(0, str::len), 0)?;
        }
    }
    Ok(bound)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Arguments::parse();
    let mut budget = ResourceBudget::new(args.bytes, args.rows, args.steps);
    let image = NativeImage::capture(
        &mut File::open(&args.database)?,
        CatalogScope::new(1),
        &mut budget,
        &|| false,
    )?;
    let table = image
        .catalog()
        .catalog()
        .lookup(&args.table, Some(ObjectKind::Table), None)?
        .filter(|table| table.name().as_str() == args.table)
        .ok_or(InspectError::UnknownTable)?;
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == table.id())
        .ok_or(InspectError::UnknownObject)?;
    let extra = image.inspect_table_extra(table.id(), &mut budget, &|| false)?;
    let mut output = Output::new(
        output_bound(&extra, &args.table, &mut budget)?,
        args.output_bytes,
    )?;
    writeln!(output, "TABLE_EXTRA\t1")?;
    write!(
        output,
        "OBJECT\t{}\t{}\t{}\t{}\t{}\t{}\t",
        extra.native_id(),
        extra.parent_id(),
        extra.catalog_page(),
        extra.catalog_slot(),
        origin.type_code(),
        origin.flags()
    )?;
    output.hex(args.table.as_bytes())?;
    writeln!(output)?;
    write!(
        output,
        "PAYLOAD\t{:?}\t{:?}\t",
        extra.presence(),
        extra.coverage()
    )?;
    if let Some(raw) = extra.raw() {
        output.hex(raw)?;
    } else {
        write!(output, "-")?;
    }
    writeln!(output)?;
    for (index, name) in extra.names().iter().enumerate() {
        write!(output, "NAME\t{index}\t")?;
        output.hex(name.as_bytes())?;
        writeln!(output)?;
    }
    for (index, group) in extra.groups().iter().enumerate() {
        write!(
            output,
            "GROUP\t{index}\t{}\t{}\t",
            group.kind(),
            group.properties().is_some()
        )?;
        output.hex(group.raw())?;
        writeln!(output)?;
        for (position, property) in group.properties().unwrap_or(&[]).iter().enumerate() {
            write!(
                output,
                "PROPERTY\t{index}\t{position}\t{}\t{}\t",
                property.flags(),
                property.native_type()
            )?;
            output.hex(property.name().as_bytes())?;
            write!(output, "\t")?;
            output.hex(property.raw())?;
            write!(output, "\t")?;
            if let Some(text) = property.text() {
                output.hex(text.as_bytes())?;
            } else {
                write!(output, "-")?;
            }
            writeln!(output)?;
        }
    }
    let mut stdout = io::stdout().lock();
    stdout.write_all(&output.bytes)?;
    stdout.flush()?;
    Ok(())
}
