//! Disposable ordinary-row INSERT candidate; never a native save.
#[path = "support/fixed_values.rs"]
mod fixed_values;
#[path = "preview_ace16_insert/input.rs"]
mod input;
#[path = "preview_ace16_slot/output.rs"]
mod output;

use clap::Parser;
use std::{
    error::Error,
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use tablecraft_ace::{AutoNumberRowInsert, IncrementingAutoNumber, NativeImage, RowInsert};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const BYTE_BUDGET: u64 = 256 * 1024 * 1024;
const ROW_BUDGET: u64 = 100_000;
const STEP_BUDGET: u64 = 100_000_000;
const REQUEST_VALUE_BYTES: usize = 1024;

#[derive(Parser)]
#[command(about = "Create an unqualified ACE 16 ordinary-row INSERT candidate")]
struct Args {
    source: PathBuf,
    output: PathBuf,
    table: String,
    /// Headerless quoted TSV: one KIND/DATA pair per manual column in native
    /// order, or per companion column with --incrementing-last;
    /// 16 MiB and 4096 complete rows, under one shared operation budget.
    #[arg(value_name = "TSV")]
    requests: PathBuf,
    /// Maximum encoded TSV DATA bytes per value; Binary counts hex digits.
    /// The 16 MiB file ceiling and shared operation budget still apply.
    #[arg(long, value_name = "BYTES", default_value_t = REQUEST_VALUE_BYTES, value_parser = input::value_bytes)]
    value_bytes: usize,
    /// Select incrementing AutoNumber engineering behavior and pin the original
    /// LAST-issued counter. TSV contains only companions, in native order,
    /// excluding the generated primary column. Stored New Values mode is not detected.
    #[arg(long, value_name = "LAST", allow_hyphen_values = true)]
    incrementing_last: Option<i32>,
}

enum Requests {
    Explicit(Vec<RowInsert>),
    Incrementing {
        rows: Vec<AutoNumberRowInsert>,
        policy: IncrementingAutoNumber,
    },
}

#[derive(Debug)]
struct Generated {
    column: usize,
    values: Vec<i32>,
}

#[derive(Debug)]
struct Published {
    changed_rows: usize,
    generated: Option<Generated>,
}

fn run(args: Args, budget: &mut ResourceBudget) -> Result<Published, Box<dyn Error>> {
    let requests = if let Some(last) = args.incrementing_last {
        Requests::Incrementing {
            rows: input::load(
                &args.requests,
                budget,
                args.value_bytes,
                AutoNumberRowInsert::new,
            )?,
            policy: IncrementingAutoNumber::new(last),
        }
    } else {
        Requests::Explicit(input::load(
            &args.requests,
            budget,
            args.value_bytes,
            RowInsert::new,
        )?)
    };
    let mut source = File::open(&args.source)?;
    let image = NativeImage::capture(&mut source, CatalogScope::new(1), budget, &|| false)?;

    match requests {
        Requests::Explicit(rows) => {
            let candidate =
                image.preview_ace16_insert_rows(&args.table, &rows, budget, &|| false)?;
            output::publish(&args.output, candidate.candidate_bytes())?;
            Ok(Published {
                changed_rows: candidate.changed_rows(),
                generated: None,
            })
        }
        Requests::Incrementing { rows, policy } => {
            let candidate = image.preview_ace16_incrementing_insert_rows(
                &args.table,
                &rows,
                policy,
                budget,
                &|| false,
            )?;
            let generated = candidate.generated_values();
            budget.charge_allocation(generated.len() as u64, size_of::<i32>() as u64)?;
            budget.charge(Resource::Steps, generated.len() as u64)?;
            let mut values = Vec::new();
            values.try_reserve_exact(generated.len())?;
            values.extend_from_slice(generated);
            output::publish(&args.output, candidate.candidate_bytes())?;
            Ok(Published {
                changed_rows: candidate.inserted_rows(),
                generated: Some(Generated {
                    column: candidate.generated_column_number(),
                    values,
                }),
            })
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let mut budget = ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET);
    let published = run(args, &mut budget)?;
    write_report(io::stdout().lock(), published)?;
    eprintln!(
        "Engineering candidate: data-macro semantics and native Access qualification pending."
    );
    Ok(())
}

fn write_report(mut output: impl Write, published: Published) -> io::Result<()> {
    writeln!(output, "candidate_changed_rows={}", published.changed_rows)?;
    if let Some(generated) = published.generated {
        writeln!(output, "generated_column_number={}", generated.column)?;
        for (request, value) in generated.values.iter().enumerate() {
            writeln!(output, "generated_value[{request}]={value}")?;
        }
    }
    output.flush()
}

#[cfg(test)]
#[path = "preview_ace16_insert/tests.rs"]
mod tests;
