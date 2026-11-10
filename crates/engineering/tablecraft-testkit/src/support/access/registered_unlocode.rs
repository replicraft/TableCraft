//! Explicit private-source selection for the recorded UN/LOCODE reader sample.
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf};

const SOURCE_BYTES: u64 = 10_956_800;
const SOURCE_SHA256: &str = "e174caa2d7048a3ad22b18618af679298bb14802e515e77102289602b454c7ac";

/// Load only the registered original selected by an opt-in scenario.
///
/// Its `.mdb` suffix does not imply Jet 4: this sample contains ACE 2007 bytes.
/// No source values or machine-specific path are embedded in the test suite.
pub fn load_from_environment() -> (PathBuf, Vec<u8>) {
    let source = PathBuf::from(
        env::var_os("TABLECRAFT_UNLOCODE_SOURCE")
            .expect("explicit registered UN/LOCODE source path"),
    );
    assert_eq!(fs::metadata(&source).unwrap().len(), SOURCE_BYTES);
    let bytes = fs::read(&source).unwrap();
    assert_eq!(bytes.len() as u64, SOURCE_BYTES);
    let hash = crate::support::bytes::hex(&Sha256::digest(&bytes));
    assert_eq!(hash, SOURCE_SHA256);

    (source, bytes)
}
