//! Read-only range example for docs/testing/IndexFixture.java's Indexed table.
use clap::{Parser, Subcommand, ValueEnum};
use std::{fs::File, ops::Bound, path::PathBuf};
use tablecraft_ace::{
    Int32IndexSelection, Int64IndexSelection, read_table_index_range, read_table_int64_index_range,
};
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
    #[arg(long, default_value = "Indexed")]
    table: String,
    #[arg(long, value_enum, default_value_t=Width::Int32)]
    width: Width,
    #[command(subcommand)]
    selection: Selection,
}

#[derive(Clone, Copy, ValueEnum)]
enum Width {
    Int32,
    Int64,
}

#[derive(Subcommand)]
enum Selection {
    All,
    Null,
    Range {
        #[arg(long, allow_negative_numbers = true)]
        lower: Option<i64>,
        #[arg(long, requires = "lower")]
        exclude_lower: bool,
        #[arg(long, allow_negative_numbers = true)]
        upper: Option<i64>,
        #[arg(long, requires = "upper")]
        exclude_upper: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let selection = match args.selection {
        Selection::All => Int64IndexSelection::All,
        Selection::Null => Int64IndexSelection::Null,
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
            Int64IndexSelection::Range {
                lower: bound(lower, exclude_lower),
                upper: bound(upper, exclude_upper),
            }
        }
    };
    let mut source = File::open(args.database)?;
    let mut budget = ResourceBudget::new(INPUT_BYTES, INPUT_ROWS, INPUT_STEPS);
    let table = match args.width {
        Width::Int64 => read_table_int64_index_range(
            &mut source,
            CatalogScope::new(1),
            &args.table,
            &args.index,
            selection,
            &mut budget,
            &|| false,
        )?,
        Width::Int32 => {
            let narrow = |bound| -> Result<_, std::num::TryFromIntError> {
                Ok(match bound {
                    Bound::Included(value) => Bound::Included(i32::try_from(value)?),
                    Bound::Excluded(value) => Bound::Excluded(i32::try_from(value)?),
                    Bound::Unbounded => Bound::Unbounded,
                })
            };
            let selection = match selection {
                Int64IndexSelection::All => Int32IndexSelection::All,
                Int64IndexSelection::Null => Int32IndexSelection::Null,
                Int64IndexSelection::Range { lower, upper } => Int32IndexSelection::Range {
                    lower: narrow(lower)?,
                    upper: narrow(upper)?,
                },
            };
            read_table_index_range(
                &mut source,
                CatalogScope::new(1),
                &args.table,
                &args.index,
                selection,
                &mut budget,
                &|| false,
            )?
        }
    };
    if table.columns().first().is_none_or(|c| c.name() != "ID")
        || table.columns().get(1).is_none_or(|c| {
            c.name()
                != match args.width {
                    Width::Int32 => "Category",
                    Width::Int64 => "Value",
                }
        })
        || table.rows().iter().any(|row| {
            !matches!(row[0], Value::Int32(_))
                || !match args.width {
                    Width::Int32 => matches!(row[1], Value::Int32(_) | Value::Null),
                    Width::Int64 => matches!(row[1], Value::Int64(_) | Value::Null),
                }
        })
    {
        return Err("example requires the generated Indexed or LargeIndexed fixture".into());
    }
    for row in table.rows() {
        let Value::Int32(id) = row[0] else {
            unreachable!("validated example carrier")
        };
        match row[1] {
            Value::Int32(category) => println!("{id}\t{category}"),
            Value::Int64(value) => println!("{id}\t{value}"),
            Value::Null => println!("{id}\tNULL"),
            _ => unreachable!("validated example carrier"),
        }
    }
    Ok(())
}
