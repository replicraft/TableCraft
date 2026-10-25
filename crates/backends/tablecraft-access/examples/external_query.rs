//! Engineering-only external IN over explicitly opened offline native handles.
//! Database tokens select host mappings; they never authorize opening files.
use base64::{display::Base64Display, engine::general_purpose::STANDARD};
use clap::{Parser, ValueEnum};
use std::{
    cell::RefCell,
    cmp::Ordering,
    fs::File,
    io::{self, Write},
    num::ParseIntError,
    path::PathBuf,
};
use tablecraft_access::{
    ExternalNativeSource, LinkedNativeSource, NativeExternalMapping, NativeLinkMapping,
    NativeSource,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_model::CatalogScope;
use tablecraft_query::{
    ExecutionOptions, QueryLimits, prepare,
    source::{CollationResolver, SourceKey},
};
use tablecraft_sql::{ParseLimits, parse_query};

const DEFAULT_NATIVE_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_NATIVE_ROWS: u64 = 16_384;
const DEFAULT_NATIVE_STEPS: u64 = 1_000_000;
const COMPARISON_BYTES: u64 = 16 * 1024 * 1024;
const COMPARISON_STEPS: u64 = 1_000_000;
const DEFAULT_OUTPUT_BYTES: usize = 1024 * 1024;
const MAPPING_FIELDS: usize = 5;

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Comparison {
    Ordinal,
    Database,
}

#[derive(Parser)]
struct Arguments {
    frontend: PathBuf,
    backend: PathBuf,
    #[arg(required_unless_present = "saved", conflicts_with = "saved")]
    sql: Option<String>,
    #[arg(long, conflicts_with = "sql")]
    saved: Option<String>,
    /// ALIAS SIGNED_ROOT_ID EXPECTED_DATABASE TARGET_TABLE SIGNED_TARGET_ID.
    #[arg(long, num_args = MAPPING_FIELDS, allow_hyphen_values = true)]
    mapping: Vec<String>,
    /// Exact database token mapped to the already-open backend. Repeat explicitly.
    #[arg(long)]
    external: Vec<String>,
    #[arg(long, value_enum, default_value = "ordinal")]
    collation: Comparison,
    #[arg(long, default_value_t = DEFAULT_NATIVE_BYTES)]
    native_bytes: u64,
    #[arg(long, default_value_t = DEFAULT_NATIVE_ROWS)]
    native_rows: u64,
    #[arg(long, default_value_t = DEFAULT_NATIVE_STEPS)]
    native_steps: u64,
    #[arg(long, default_value_t = QueryLimits::DEFAULT_ROWS)]
    query_rows: u64,
    #[arg(long, default_value_t = DEFAULT_OUTPUT_BYTES)]
    output_bytes: usize,
}

struct Output {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("external query output limit exceeded"));
        }
        self.bytes
            .try_reserve_exact(bytes.len())
            .map_err(|_| io::Error::other("external query output allocation failed"))?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    let (groups, remainder) = args.mapping.as_chunks::<MAPPING_FIELDS>();
    if !remainder.is_empty() {
        return Err("each supplied mapping requires five fields".into());
    }
    if args.output_bytes > DEFAULT_OUTPUT_BYTES {
        return Err("engineering output ceiling exceeded".into());
    }
    let native = RefCell::new(ResourceBudget::new(
        args.native_bytes,
        args.native_rows,
        args.native_steps,
    ));
    let comparisons = RefCell::new(ResourceBudget::new(COMPARISON_BYTES, 0, COMPARISON_STEPS));
    let cancelled = || false;

    // Explicit host paths are the only opens. Stored Database/Connect fields
    // are never parsed, joined to paths, resolved or used as credentials.
    let root = NativeSource::new_with_shared_budget(
        File::open(&args.frontend)?,
        CatalogScope::new(1),
        &native,
        &cancelled,
    )?;
    let target = NativeSource::new_with_shared_budget(
        File::open(&args.backend)?,
        CatalogScope::new(2),
        &native,
        &cancelled,
    )?;
    let (mut root, root_policy) = match args.collation {
        Comparison::Ordinal => (root, None),
        Comparison::Database => {
            let (root, policy) = root.with_database_collation_resolver(&comparisons)?;
            (root, Some(policy))
        }
    };
    let (target, _target_policy) = match args.collation {
        Comparison::Ordinal => (target, None),
        Comparison::Database => {
            let (target, policy) = target.with_database_collation_resolver(&comparisons)?;
            (target, Some(policy))
        }
    };
    let saved = args
        .saved
        .as_deref()
        .map(|name| root.read_saved_query(name, ParseLimits::default()))
        .transpose()?;
    let parsed = args
        .sql
        .as_deref()
        .map(|sql| parse_query(sql, ParseLimits::default()))
        .transpose()?;
    let query = match (&saved, &parsed) {
        (Some(saved), None) => saved.query(),
        (None, Some(parsed)) => parsed.query(),
        _ => return Err("exactly one SQL or saved root is required".into()),
    };
    let mappings = groups
        .iter()
        .map(|fields| {
            Ok(NativeLinkMapping::new(
                &fields[0],
                fields[1].parse()?,
                &fields[2],
                target.handle(),
                &fields[3],
                fields[4].parse()?,
            ))
        })
        .collect::<Result<Vec<_>, ParseIntError>>()?;
    let linked = LinkedNativeSource::new(root.handle(), &mappings)?;
    let externals = args
        .external
        .iter()
        .enumerate()
        .map(|(position, token)| {
            NativeExternalMapping::new(token, SourceKey::new(position as u128), target.handle())
        })
        .collect::<Vec<_>>();
    let source = ExternalNativeSource::with_links(&linked, &externals)?;
    let ordinal = Ordinal;
    let options = ExecutionOptions {
        collation: matches!(args.collation, Comparison::Ordinal)
            .then_some(&ordinal as &dyn Collation),
        collation_resolver: root_policy
            .as_ref()
            .map(|policy| policy as &dyn CollationResolver),
        limits: QueryLimits {
            rows: args.query_rows,
            ..QueryLimits::default()
        },
        ..ExecutionOptions::default()
    };
    let result = prepare(query, &source)?.execute(&source, &[], options)?;

    let mut output = Output {
        bytes: Vec::new(),
        limit: args.output_bytes,
    };
    for (position, column) in result.columns().iter().enumerate() {
        writeln!(
            output,
            "C\t{position}\tT{}",
            Base64Display::new(column.as_bytes(), &STANDARD)
        )?;
    }
    for (row_number, row) in result.rows().iter().enumerate() {
        for (column, value) in row.iter().enumerate() {
            write!(output, "R\t{row_number}\t{column}\t")?;
            match value {
                Value::Null => writeln!(output, "N")?,
                Value::Int16(value) => writeln!(output, "I{value}")?,
                Value::Int32(value) => writeln!(output, "I{value}")?,
                Value::Int64(value) => writeln!(output, "I{value}")?,
                Value::Double(value) => writeln!(output, "F{:016x}", value.to_bits())?,
                Value::Text(value) => writeln!(
                    output,
                    "T{}",
                    Base64Display::new(value.as_bytes(), &STANDARD)
                )?,
                _ => {
                    return Err("engineering result carrier is outside this fixture profile".into());
                }
            }
        }
    }
    io::stdout().lock().write_all(&output.bytes)?;
    Ok(())
}
