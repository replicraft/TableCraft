//! Development-only reproduction of the independently observed rename pairs.
//!
//! This accepts only a fresh output directory, never an existing user database.
//! Every input comes from the independent raw testkit builder. The real standard
//! capture and native planner must reproduce its complete BEFORE/AFTER bytes
//! before any pair is exported. The original seven cases and header 5/6 names
//! match the retained read-only SDK observation set. No SDK, native Access,
//! transaction or production CLI/backend write route is invoked here. This does
//! not broaden native operation admission.
//! One finite cumulative budget covers native capture, planning, comparisons and
//! export writes. A failure can leave a disposable prefix in the newly created
//! directory; no existing destination or file is replaced.
use clap::Parser;
use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Cursor},
    path::{Path, PathBuf},
};
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_catalog_names::rename_pair;

const BYTE_BUDGET: u64 = 512 * 1024 * 1024;
const ROW_BUDGET: u64 = 1_000_000;
const STEP_BUDGET: u64 = 100_000_000;
const COMPARISON_CHUNK_BYTES: usize = 64 * 1024;
const SCOPE: CatalogScope = CatalogScope::new(1);

#[derive(Parser)]
#[command(about = "Export checked native table-rename fixture pairs into a fresh directory")]
struct Arguments {
    /// A new disposable directory; its parent must already exist.
    #[arg(value_name = "NEW_DIRECTORY")]
    destination: PathBuf,
}

fn compare(
    actual: &[u8],
    independent: &[u8],
    budget: &mut ResourceBudget,
) -> Result<(), Box<dyn Error>> {
    if actual.len() != independent.len() {
        return Err(
            io::Error::new(io::ErrorKind::InvalidData, "fixture image lengths differ").into(),
        );
    }
    for (actual, independent) in actual
        .chunks(COMPARISON_CHUNK_BYTES)
        .zip(independent.chunks(COMPARISON_CHUNK_BYTES))
    {
        budget.charge(Resource::Steps, actual.len() as u64)?;
        if actual != independent {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "complete fixture images differ",
            )
            .into());
        }
    }
    Ok(())
}

fn export(path: &Path, bytes: &[u8], budget: &mut ResourceBudget) -> Result<(), Box<dyn Error>> {
    budget.charge(Resource::Bytes, bytes.len() as u64)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    tablecraft_io::write_exact_at(&mut file, 0, bytes, budget, &|| false)?;
    file.sync_all()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Arguments::parse();
    fs::create_dir(&args.destination)?;
    let mut budget = ResourceBudget::new(BYTE_BUDGET, ROW_BUDGET, STEP_BUDGET);
    let boundary = "表".repeat(64);
    let cases = [
        ("Items", "Renamed", true),
        ("LongerSource", "I", true),
        ("Items", "雪 Customers", true),
        ("Items", "items", false),
        ("\u{feff}雪🙂", "\u{feff}āΩ雪🙂A", true),
        ("Items", "Items", false),
        ("Items", boundary.as_str(), true),
    ];
    let mut pairs = 0;
    for header in [5, 6] {
        for (index, (old_name, new_name, populated)) in cases.iter().enumerate() {
            let pair = rename_pair(header, old_name, new_name, *populated);
            let image = NativeImage::capture(
                &mut Cursor::new(pair.before.as_slice()),
                SCOPE,
                &mut budget,
                &|| false,
            )?;
            let plan = image.plan_table_rename(old_name, new_name, &mut budget, &|| false)?;
            compare(plan.before_bytes(), &pair.before, &mut budget)?;
            compare(plan.after_bytes(), &pair.after, &mut budget)?;

            let prefix = format!("header-{header}-case-{index:02}");
            export(
                &args.destination.join(format!("{prefix}-before.accdb")),
                plan.before_bytes(),
                &mut budget,
            )?;
            export(
                &args.destination.join(format!("{prefix}-after.accdb")),
                plan.after_bytes(),
                &mut budget,
            )?;
            export(
                &args.destination.join(format!("{prefix}-old.txt")),
                pair.old_name.as_bytes(),
                &mut budget,
            )?;
            export(
                &args.destination.join(format!("{prefix}-new.txt")),
                pair.new_name.as_bytes(),
                &mut budget,
            )?;
            export(
                &args.destination.join(format!("{prefix}-rows.txt")),
                pair.rows.len().to_string().as_bytes(),
                &mut budget,
            )?;
            pairs += 1;
        }
    }
    println!("Exported {pairs} checked native BEFORE/AFTER pairs matching independent raw images");
    Ok(())
}
