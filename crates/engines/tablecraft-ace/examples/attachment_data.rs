//! Engineering-only exact original attachment bytes for the complex-row recipe.
use clap::Parser;
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use tablecraft_ace::{decode_attachment, read_complex_column_links};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const BYTES: u64 = 16 * 1024 * 1024;
const ROWS: u64 = 16_384;
const STEPS: u64 = 1_000_000;
const OUTPUT_BYTES: usize = 1024 * 1024;
const ROW_FORMAT_BYTES: usize = 64;

#[derive(Parser)]
struct Arguments {
    database: PathBuf,
    #[arg(long, default_value = "Collections")]
    table: String,
    #[arg(long, default_value = "Files")]
    column: String,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let mut budget = ResourceBudget::new(BYTES, ROWS, STEPS);
    let links = read_complex_column_links(
        &mut File::open(args.database)?,
        CatalogScope::new(1),
        &args.table,
        &args.column,
        &mut budget,
        &|| false,
    )?;
    let position = |name| {
        links
            .child_snapshot()
            .columns()
            .iter()
            .position(|column| column.name() == name)
            .ok_or("example requires the generated attachment schema")
    };
    let data = position("FileData")?;
    let identity = links
        .child_snapshot()
        .schema()
        .columns()
        .iter()
        .position(|column| column.number() == links.column().identity_number())
        .ok_or("missing child identity")?;
    let mut output = Vec::new();
    for parent in 0..links.parents().len() {
        for row in links.child_rows(parent).ok_or("invalid parent ordinal")? {
            let (Value::Binary(bytes), Value::Int32(id)) = (&row[data], &row[identity]) else {
                return Err("example requires exact attachment storage".into());
            };
            let decoded = decode_attachment(bytes, &mut budget, &|| false)?;
            let allowance =
                2 * (decoded.type_label().len() + decoded.data().len()) + ROW_FORMAT_BYTES;
            if output.len() + allowance > OUTPUT_BYTES {
                return Err("engineering output limit exceeded".into());
            }
            budget.charge_allocation(allowance as u64, 1)?;
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
