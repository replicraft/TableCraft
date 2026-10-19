//! Engineering-only reciprocal-index metadata comparison. No rows or writes.
use clap::Parser;
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use tablecraft_ace::inspect_foreign_index_links;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ROWS: u64 = 16_384;
const MAX_STEPS: u64 = 1_000_000;

#[derive(Parser)]
struct Arguments {
    database: PathBuf,
    table: String,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let result = inspect_foreign_index_links(
        &mut File::open(args.database)?,
        CatalogScope::new(1),
        &args.table,
        &mut ResourceBudget::new(MAX_BYTES, MAX_ROWS, MAX_STEPS),
        &|| false,
    )?;
    let mut output = Vec::new();
    for link in result.links() {
        let index = result
            .indexes()
            .iter()
            .find(|i| i.number() == link.index_number())
            .ok_or("missing local index")?;
        let reference = index.foreign_reference().ok_or("missing reference")?;
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            result.schema().name(),
            index.name(),
            index.number(),
            reference.native_role(),
            reference.table_page(),
            reference.index_number(),
            reference.update_flags(),
            reference.delete_flags(),
            link.referenced_index().name()
        )?;
    }
    io::stdout().lock().write_all(&output)?;
    Ok(())
}
