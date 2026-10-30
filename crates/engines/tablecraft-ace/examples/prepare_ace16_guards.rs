//! Explicit read-only engineering guard preparation; never creates a candidate.
use clap::{Parser, Subcommand};
use std::{
    fmt::Write as _,
    fs::File,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};
use tablecraft_ace::{
    BeforeEventGuardSelector, BeforeEventIntents, FixedScalarUpdate, NativeImage, RowDelete,
    RowInsert,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const BYTE_BUDGET: u64 = 256 * 1024 * 1024;
const ROW_BUDGET: u64 = 100_000;
const STEP_BUDGET: u64 = 100_000_000;
const MAX_RECORD_HEX_BYTES: usize = 8192;
const MAX_INSERT_VALUES: usize = 1024;
const REPORT_CAPACITY: usize = 256;
const MAX_REPORT_BYTES: usize = 1_048_576;

#[derive(Parser)]
#[command(
    about = "Prepare the explicit ACE16 before-event guard subset without creating a write candidate",
    after_help = "Uses the selected documented desktop subset and provisional actual-value-change Updated policy. Native event timing, macro absence and normal-save permission are not certified."
)]
struct Args {
    source: PathBuf,
    table: String,
    #[arg(long = "guard-property", value_names = ["KIND", "NAME"], num_args = 2, required = true)]
    guard_properties: Vec<String>,
    #[command(subcommand)]
    intent: Intent,
}

#[derive(Subcommand)]
enum Intent {
    /// One exact Int32 replacement against an original live ordinal.
    Update {
        column: String,
        row: usize,
        #[arg(allow_hyphen_values = true)]
        expected: i32,
        #[arg(allow_hyphen_values = true)]
        value: i32,
    },
    /// One all-explicit row, with every native field supplied as Int32.
    Insert {
        #[arg(required = true, allow_hyphen_values = true)]
        values: Vec<i32>,
    },
    /// One deletion with exact original record bytes, including NULL mask.
    Delete { row: usize, expected_hex: String },
}

fn record_hex(
    text: &str,
    budget: &mut ResourceBudget,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if text.is_empty() || text.len() > MAX_RECORD_HEX_BYTES || !text.len().is_multiple_of(2) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected an even bounded record HEX string",
        )
        .into());
    }
    budget.charge(Resource::Steps, text.len() as u64)?;
    budget.charge_allocation((text.len() / 2) as u64, 1)?;
    let mut output = Vec::new();
    output.try_reserve_exact(text.len() / 2)?;
    for pair in text.as_bytes().as_chunks::<2>().0 {
        let text = std::str::from_utf8(pair)?;
        output.push(u8::from_str_radix(text, 16)?);
    }
    Ok(output)
}

fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut budget = ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET);
    if args.guard_properties.is_empty()
        || !args.guard_properties.len().is_multiple_of(2)
        || args.guard_properties.len() / 2 > NativeImage::MAX_BEFORE_GUARD_PROPERTIES
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected bounded --guard-property KIND NAME pairs",
        )
        .into());
    }
    budget.charge_allocation(
        (args.guard_properties.len() / 2) as u64,
        size_of::<BeforeEventGuardSelector<'_>>() as u64,
    )?;
    let mut selectors = Vec::new();
    selectors.try_reserve_exact(args.guard_properties.len() / 2)?;
    for pair in args.guard_properties.as_chunks::<2>().0 {
        budget.charge(Resource::Steps, (pair[0].len() + pair[1].len()) as u64)?;
        let group = pair[0].parse::<u16>()?;
        if pair[1].is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "guard property name is empty",
            )
            .into());
        }
        selectors.push(BeforeEventGuardSelector::new(group, &pair[1]));
    }
    let mut updates = Vec::new();
    let mut inserts = Vec::new();
    let mut deletes = Vec::new();

    // Typed request/input validation precedes opening the database. The source
    // is captured once; no existing mutation preview or active-content bypass runs.
    match &args.intent {
        Intent::Update {
            row,
            expected,
            value,
            ..
        } => {
            budget.charge_allocation(1, size_of::<FixedScalarUpdate>() as u64)?;
            updates.try_reserve_exact(1)?;
            updates.push(FixedScalarUpdate::new(
                *row,
                Value::Int32(*expected),
                Value::Int32(*value),
            ));
        }
        Intent::Insert { values } => {
            if values.len() > MAX_INSERT_VALUES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "too many explicit row values",
                )
                .into());
            }
            budget.charge_allocation(values.len() as u64, size_of::<Value>() as u64)?;
            let mut row = Vec::new();
            row.try_reserve_exact(values.len())?;
            row.extend(values.iter().copied().map(Value::Int32));
            budget.charge_allocation(1, size_of::<RowInsert>() as u64)?;
            inserts.try_reserve_exact(1)?;
            inserts.push(RowInsert::new(row));
        }
        Intent::Delete { row, expected_hex } => {
            let expected = record_hex(expected_hex, &mut budget)?;
            budget.charge_allocation(1, size_of::<RowDelete>() as u64)?;
            deletes.try_reserve_exact(1)?;
            deletes.push(RowDelete::new(*row, expected));
        }
    }
    let original = match &args.intent {
        Intent::Update { column, .. } => BeforeEventIntents::FixedUpdate {
            column,
            updates: &updates,
        },
        Intent::Insert { .. } => BeforeEventIntents::Insert(&inserts),
        Intent::Delete { .. } => BeforeEventIntents::Delete(&deletes),
    };

    let mut source = File::open(&args.source)?;
    let image = NativeImage::capture(&mut source, CatalogScope::new(1), &mut budget, &|| false)?;
    let prepared = image.prepare_ace16_before_event_guards(
        &args.table,
        &selectors,
        original,
        &mut budget,
        &|| false,
    )?;

    let capacity = prepared
        .programs()
        .iter()
        .try_fold(REPORT_CAPACITY, |total, program| {
            program
                .name()
                .len()
                .checked_mul(10)
                .and_then(|name| name.checked_add(80))
                .and_then(|row| total.checked_add(row))
        })
        .filter(|bytes| *bytes <= MAX_REPORT_BYTES)
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "guard report limit exceeded")
        })?;
    budget.charge_allocation(capacity as u64, 1)?;
    budget.charge(Resource::Steps, capacity as u64)?;
    let mut report = String::new();
    report.try_reserve_exact(capacity)?;
    for (index, program) in prepared.programs().iter().enumerate() {
        writeln!(
            &mut report,
            "selected_guard_{index}={}:{:?}",
            program.group(),
            program.name()
        )?;
    }
    if let Some(rejection) = prepared.rejection() {
        writeln!(
            &mut report,
            "guard_status=rejected\nguard_error_number={}",
            rejection.number()
        )?;
    } else {
        writeln!(&mut report, "guard_status=passed")?;
    }
    writeln!(
        &mut report,
        "visited_steps={}\nnormal_save_authorized=false",
        prepared.trace().len()
    )?;
    budget.charge(Resource::Bytes, report.len() as u64)?;
    let mut stdout = io::stdout().lock();
    stdout.write_all(report.as_bytes())?;
    stdout.flush()?;
    Ok(if prepared.rejection().is_some() {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_request_shape_and_exact_hex_parse_without_opening_files() {
        for text in ["", "0", "GG", "α0"] {
            assert!(record_hex(text, &mut ResourceBudget::new(1_000_000, 0, 1_000_000)).is_err());
        }
        assert_eq!(
            record_hex(
                "0200010000000A00000003",
                &mut ResourceBudget::new(1_000_000, 0, 1_000_000)
            )
            .unwrap(),
            [2, 0, 1, 0, 0, 0, 10, 0, 0, 0, 3]
        );
        assert!(
            Args::try_parse_from([
                "guards",
                "missing.accdb",
                "Items",
                "--guard-property",
                "3",
                "StoredGuard",
                "insert"
            ])
            .is_err()
        );
        assert!(
            Args::try_parse_from([
                "guards",
                "missing.accdb",
                "Items",
                "--guard-property",
                "3",
                "StoredGuard",
                "update",
                "Score",
                "0",
                "10"
            ])
            .is_err()
        );
        assert!(
            Args::try_parse_from([
                "guards",
                "missing.accdb",
                "Items",
                "--guard-property",
                "3",
                "StoredGuard",
                "update",
                "Score",
                "0",
                "-1",
                "0"
            ])
            .is_ok()
        );
    }
}
