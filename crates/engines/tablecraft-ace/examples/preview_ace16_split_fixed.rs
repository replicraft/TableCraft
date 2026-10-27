//! Fresh, non-journalable exact integral/Currency split candidate; never a native save.
#[path = "preview_ace16_slot/output.rs"]
mod output;

#[path = "support/fixed_values.rs"]
mod fixed_values;

use clap::{Parser, ValueEnum};
use std::{error::Error, fs::File, path::PathBuf};
use tablecraft_ace::{FixedScalarUpdate, NativeImage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

const BYTE_BUDGET: u64 = 256 * 1024 * 1024;
const ROW_BUDGET: u64 = 100_000;
const STEP_BUDGET: u64 = 100_000_000;

#[derive(Parser)]
#[command(about = "Create an unqualified ACE 16 exact fixed scalar split candidate")]
struct Args {
    source: PathBuf,
    output: PathBuf,
    table: String,
    column: String,
    row: usize,
    #[arg(value_enum)]
    kind: Kind,
    #[arg(allow_hyphen_values = true)]
    expected: String,
    #[arg(allow_hyphen_values = true)]
    value: String,
}

#[derive(Clone, Copy, ValueEnum)]
enum Kind {
    Byte,
    Int16,
    Int32,
    Int64,
    CurrencyScaled,
}

impl Kind {
    fn parse(self, text: &str) -> Result<Value, &'static str> {
        let kind = match self {
            Self::Byte => fixed_values::Kind::Byte,
            Self::Int16 => fixed_values::Kind::Int16,
            Self::Int32 => fixed_values::Kind::Int32,
            Self::Int64 => fixed_values::Kind::Int64,
            Self::CurrencyScaled => fixed_values::Kind::CurrencyScaled,
        };
        let value = fixed_values::parse(kind, text)?;
        if value == Value::Null {
            return Err("NULL requests are outside the fixed split profile");
        }
        Ok(value)
    }
}

fn run(args: Args, budget: &mut ResourceBudget) -> Result<usize, Box<dyn Error>> {
    let request = FixedScalarUpdate::new(
        args.row,
        args.kind.parse(&args.expected)?,
        args.kind.parse(&args.value)?,
    );
    let mut source = File::open(&args.source)?;
    let image = NativeImage::capture(&mut source, CatalogScope::new(1), budget, &|| false)?;
    let preview = image.preview_ace16_split_fixed_updates(
        &args.table,
        &args.column,
        &[request],
        budget,
        &|| false,
    )?;
    output::publish(&args.output, preview.candidate_bytes())?;
    Ok(preview.changed_rows())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let mut budget = ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET);
    let changed = run(args, &mut budget)?;
    println!("candidate_changed_rows={changed}");
    eprintln!(
        "Engineering candidate: data-macro semantics and native Access qualification pending."
    );
    Ok(())
}

#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/mutation/fixture.rs"]
mod fixture;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/mutation/index_fixture.rs"]
mod index_fixture;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/ace16_split_int32/model.rs"]
mod model;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/support/mod.rs"]
mod support;
#[cfg(test)]
#[path = "preview_ace16_split_fixed/tests.rs"]
mod tests;

#[cfg(test)]
use tablecraft_ace as engine;
