//! Read-only range example for docs/testing/IndexFixture.java's Indexed table.
use clap::{Parser, Subcommand};
use std::{fs::File, ops::Bound, path::PathBuf};
use tablecraft_ace::{Int32IndexSelection, read_table_index_range};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const INPUT_BYTES: u64 = 16 * 1024 * 1024;
const INPUT_ROWS: u64 = 16_384;
const INPUT_STEPS: u64 = 1_000_000;

#[derive(Parser)]
struct Arguments {
    database: PathBuf,
    #[arg(long, default_value = "PrimaryKey")]
    index: String,
    #[command(subcommand)]
    selection: Selection,
}

#[derive(Subcommand)]
enum Selection {
    All,
    Null,
    Range {
        #[arg(long, allow_negative_numbers = true)]
        lower: Option<i32>,
        #[arg(long, requires = "lower")]
        exclude_lower: bool,
        #[arg(long, allow_negative_numbers = true)]
        upper: Option<i32>,
        #[arg(long, requires = "upper")]
        exclude_upper: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let selection = match args.selection {
        Selection::All => Int32IndexSelection::All,
        Selection::Null => Int32IndexSelection::Null,
        Selection::Range {
            lower,
            upper,
            exclude_lower,
            exclude_upper,
        } => {
            let bound = |value, exclusive| match (value, exclusive) {
                (None, _) => Bound::Unbounded,
                (Some(value), true) => Bound::Excluded(value),
                (Some(value), false) => Bound::Included(value),
            };
            Int32IndexSelection::Range {
                lower: bound(lower, exclude_lower),
                upper: bound(upper, exclude_upper),
            }
        }
    };
    let table = read_table_index_range(
        &mut File::open(args.database)?,
        CatalogScope::new(1),
        "Indexed",
        &args.index,
        selection,
        &mut ResourceBudget::new(INPUT_BYTES, INPUT_ROWS, INPUT_STEPS),
        &|| false,
    )?;
    if table.columns().first().is_none_or(|c| c.name() != "ID")
        || table
            .columns()
            .get(1)
            .is_none_or(|c| c.name() != "Category")
        || table.rows().iter().any(|row| {
            !matches!(row[0], Value::Int32(_)) || !matches!(row[1], Value::Int32(_) | Value::Null)
        })
    {
        return Err("example requires the generated Indexed fixture".into());
    }
    for row in table.rows() {
        let Value::Int32(id) = row[0] else {
            unreachable!("validated example carrier")
        };
        match row[1] {
            Value::Int32(category) => println!("{id}\t{category}"),
            Value::Null => println!("{id}\tNULL"),
            _ => unreachable!("validated example carrier"),
        }
    }
    Ok(())
}
