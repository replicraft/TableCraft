//! Engineering-only bounded saved SELECT reconstruction; never executes SQL.
use clap::Parser;
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use tablecraft_access::reconstruct_saved_select;
use tablecraft_ace::read_saved_query_definition;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const BYTES: u64 = 64 * 1024 * 1024;
const ROWS: u64 = 16_384;
const STEPS: u64 = 1_000_000;
const OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Parser)]
struct Arguments {
    database: PathBuf,
    #[arg(long)]
    query: String,
}
struct Output(Vec<u8>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > OUTPUT_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other("saved SELECT output limit exceeded"));
        }
        self.0
            .try_reserve_exact(bytes.len())
            .map_err(|_| io::Error::other("saved SELECT output allocation failed"))?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let mut budget = ResourceBudget::new(BYTES, ROWS, STEPS);
    let definition = read_saved_query_definition(
        &mut File::open(args.database)?,
        CatalogScope::new(1),
        &args.query,
        &mut budget,
        &|| false,
    )?;
    let saved = reconstruct_saved_select(definition, Default::default(), &mut budget, &|| false)?;
    let mut output = Output(Vec::new());
    writeln!(output, "{}", saved.query())?;
    io::stdout().lock().write_all(&output.0)?;
    Ok(())
}
