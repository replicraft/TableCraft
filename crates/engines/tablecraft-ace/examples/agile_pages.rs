//! Engineering password verification/raw-page comparison; no table-read qualification.
use clap::Parser;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};
use tablecraft_ace::{inspect_header, read_agile_encryption};
use tablecraft_core::limits::ResourceBudget;
use zeroize::Zeroizing;

const PAGE_BYTES: usize = 4096;
const MAX_PASSWORD_UTF8_BYTES: usize = 1024;
const READ_BUDGET: u64 = 32 * 1024 * 1024;
const WORK_BUDGET: u64 = 1_000_000;
const OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Parser)]
struct Args {
    /// Explicit stable/offline source; original bytes are never changed.
    path: PathBuf,
    /// Decode only this data page; by default compare every data page.
    #[arg(long)]
    page: Option<u32>,
    /// Only verify the password, without reading any data pages.
    #[arg(long, conflicts_with = "page")]
    verify_only: bool,
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::open(args.path)?;
    let mut budget = ResourceBudget::new(READ_BUDGET, 0, WORK_BUDGET);
    let header = inspect_header(&mut file, &mut budget, &|| false)?;
    let encrypted = read_agile_encryption(&mut file, &mut budget, &|| false)?;
    // Explicit UTF-8 single-line stdin input, never an argument/environment/log value.
    let mut password = Zeroizing::new([0; MAX_PASSWORD_UTF8_BYTES]);
    let mut used = 0;
    let mut stdin = std::io::stdin().lock();
    let mut byte = Zeroizing::new([0]);
    let mut supplied = false;
    while stdin.read(&mut *byte)? != 0 {
        supplied = true;
        if byte[0] == b'\n' {
            break;
        }
        if used == password.len() {
            return Err("password input exceeds the engineering bound".into());
        }
        password[used] = byte[0];
        used += 1;
    }
    if used > 0 && password[used - 1] == b'\r' {
        used -= 1;
    }
    let credential = if supplied {
        Some(std::str::from_utf8(&password[..used]).map_err(|_| "password input is not UTF-8")?)
    } else {
        None
    };
    let decoder = encrypted.unlock(credential, &mut budget, &|| false)?;
    if args.verify_only {
        std::io::stdout().lock().write_all(b"Password verified\n")?;
        return Ok(());
    }
    let pages = match args.page {
        Some(p) => p..p.checked_add(1).ok_or("invalid page ordinal")?,
        None => 1..header.page_count(),
    };
    let mut output = Vec::new();
    for number in pages {
        let mut page = Zeroizing::new([0; PAGE_BYTES]);
        if number == 0 || number >= header.page_count() {
            return Err(tablecraft_ace::InspectError::MalformedEncryption.into());
        }
        budget.charge(tablecraft_core::limits::Resource::Bytes, PAGE_BYTES as u64)?;
        tablecraft_io::read_exact_at(
            &mut file,
            u64::from(number) * PAGE_BYTES as u64,
            &mut *page,
            &mut budget,
            &|| false,
        )?;
        decoder.decode_page(number, &mut *page, &mut budget, &|| false)?;
        let digest = Sha256::digest(page.as_slice());
        let mut line = format!("{number}\t");
        for byte in digest {
            write!(&mut line, "{byte:02x}")?;
        }
        line.push('\n');
        if output.len() + line.len() > OUTPUT_BYTES {
            return Err("engineering output limit exceeded".into());
        }
        output.try_reserve(line.len())?;
        output.extend_from_slice(line.as_bytes());
    }
    std::io::stdout().lock().write_all(&output)?;
    Ok(())
}

fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
