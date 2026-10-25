//! Explicit engineering metadata oracle consumer; never opens a linked target.
use base64::{Engine, engine::general_purpose::STANDARD};
use clap::Parser;
use std::{fs::File, path::PathBuf};
use tablecraft_ace::{NativeLinkText, NativeTableStorage, read_linked_table_definition};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

#[derive(Parser)]
struct Args {
    database: PathBuf,
    name: String,
}

fn text(value: NativeLinkText<'_>) -> Result<String, Box<dyn std::error::Error>> {
    Ok(match value {
        NativeLinkText::Missing => "M".into(),
        NativeLinkText::Null => "N".into(),
        NativeLinkText::Unsupported => "U".into(),
        NativeLinkText::Text(text) => {
            let length = base64::encoded_len(text.len(), true).ok_or("Encoded text too large")?;
            let mut bytes = vec![0; length];
            let count = STANDARD
                .encode_slice(text.as_bytes(), &mut bytes)
                .map_err(|_| "Metadata encoding failed")?;
            bytes.truncate(count);
            format!("T{}", String::from_utf8(bytes)?)
        }
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut source = File::open(args.database)?;
    let mut budget = ResourceBudget::new(64 * 1024 * 1024, 16_384, 1_000_000);
    let metadata = read_linked_table_definition(
        &mut source,
        CatalogScope::new(1),
        &args.name,
        &mut budget,
        &|| false,
    )?;
    let storage = match metadata.storage() {
        NativeTableStorage::Linked => "LINKED",
        NativeTableStorage::LinkedOdbc => "LINKED_ODBC",
        NativeTableStorage::Local => return Err("Unexpected local table".into()),
    };
    let target = metadata.access_target()?;
    if !matches!(metadata.database(), NativeLinkText::Text(value) if value == target.database())
        || !matches!(metadata.foreign_name(), NativeLinkText::Text(value) if value == target.table())
    {
        return Err("Interpreted identifiers differ from stored metadata".into());
    }
    println!(
        "{}\t{}\t{}\t{}\t{}\t{}",
        text(NativeLinkText::Text(metadata.name()))?,
        metadata.native_id(),
        storage,
        text(metadata.database())?,
        text(metadata.foreign_name())?,
        text(metadata.connect())?,
    );
    Ok(())
}
