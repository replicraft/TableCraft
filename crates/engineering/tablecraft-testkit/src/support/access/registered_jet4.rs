//! Opt-in genuine V2000 source and independent whole-image edit oracle.
//!
//! The source remains private and must be selected explicitly. This helper
//! qualifies both source images through public readers before a save scenario.
use sha2::{Digest, Sha256};
use std::{env, fs, io::Cursor, path::PathBuf};
use tablecraft_ace::{FileFormat, NativeImage, check_table, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SOURCE_ENV: &str = "TABLECRAFT_JET4_SCALAR_SOURCE";
const SOURCE_SHA256: &str = "de2e67a9f0194988d031cc9c981176ae9a76d3bf80599040729b408fc2b7be82";
const EXPECTED_SHA256: &str = "abf76b9a2eec8ffd8074a2eb131cebe48d17b75691217b6e475292074108e54f";
const ID_OFFSET: usize = 130954;

/// Registered original and literal ID 1 -> 41 expected bytes.
pub struct RegisteredJet4 {
    source: PathBuf,
    /// Complete original input; save scenarios must copy these bytes first.
    pub before: Vec<u8>,
    /// Independent complete expected image, never produced by a write encoder.
    pub after: Vec<u8>,
}

fn verify_hash(bytes: &[u8], expected: &str) {
    let actual = format!("{:x}", Sha256::digest(bytes));
    assert_eq!(actual, expected);
}

fn rows(bytes: &[u8], id: i32) -> Vec<Vec<Value>> {
    let mut work = ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000);
    let scope = CatalogScope::new(1);
    let image = NativeImage::capture(&mut Cursor::new(bytes), scope, &mut work, &|| false).unwrap();
    assert_eq!(image.catalog().header().format(), FileFormat::Jet4);
    let checked = check_table(
        &mut image.reader(),
        scope,
        "ScalarSamples",
        &mut work,
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert!(checked.indexes().is_empty());
    assert_eq!(checked.schema().columns().len(), 14);
    let field = &checked.schema().columns()[0];
    assert_eq!(field.name(), "ID");
    assert_eq!(field.native_type(), 4);
    assert_eq!(field.storage_length(), 4);
    assert_eq!(field.flags(), 0x03);
    assert_eq!(field.extended_flags(), 0);
    let read = read_table(
        &mut image.reader(),
        scope,
        "ScalarSamples",
        &mut work,
        &|| false,
    )
    .unwrap();
    assert_eq!(read.rows()[0][0], Value::Int32(id));
    assert_eq!(read.rows()[1][0], Value::Int32(2));
    assert_eq!(read.rows()[2][0], Value::Int32(3));
    read.rows().to_vec()
}

/// Load only the exact registered original and qualify its literal expected image.
///
/// Requires `TABLECRAFT_JET4_SCALAR_SOURCE`; callers are explicitly ignored tests,
/// so an ordinary CI run never reads a machine cache or silently skips a case.
pub fn load_from_environment() -> RegisteredJet4 {
    let source =
        PathBuf::from(env::var_os(SOURCE_ENV).expect("explicit registered V2000 source path"));
    let before = fs::read(&source).unwrap();
    assert_eq!(before.len(), 135168);
    verify_hash(&before, SOURCE_SHA256);
    assert_eq!(&before[31 * 4096..31 * 4096 + 2], &[1, 1]);
    assert_eq!(&before[31 * 4096 + 14..31 * 4096 + 16], &[0x88, 0x0f]);
    assert_eq!(&before[ID_OFFSET - 2..ID_OFFSET + 4], &[14, 0, 1, 0, 0, 0]);
    let mut after = before.clone();
    after[ID_OFFSET..ID_OFFSET + 4].copy_from_slice(&[41, 0, 0, 0]);
    verify_hash(&after, EXPECTED_SHA256);

    let mut expected_rows = rows(&before, 1);
    expected_rows[0][0] = Value::Int32(41);
    assert_eq!(rows(&after, 41), expected_rows);
    RegisteredJet4 {
        source,
        before,
        after,
    }
}

impl RegisteredJet4 {
    /// Require that the private registered input remained unchanged by the scenario.
    pub fn assert_original_unchanged(&self) {
        assert_eq!(fs::read(&self.source).unwrap(), self.before);
    }
}
