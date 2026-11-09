//! Independent request bytes and complete raw AFTERs qualify the V2 transition.
mod codec;
mod native;
mod recovery;

use super::*;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, V2State, inspect_v2};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::{Source, repeated as raw},
    support::io::journal::MemoryStorage,
};

const SCOPE: CatalogScope = CatalogScope::new(944);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn plan(before: &[u8], table: &str, column: &str) -> RecoverableRepeatedDefaultZeroAddColumnPlan {
    image(before)
        .plan_recoverable_repeated_default_zero_int32_column(table, column, &mut budget(), &|| {
            false
        })
        .unwrap()
}

fn literal(before: &[u8], table: &str, column: &str) -> Vec<u8> {
    // Independent 60-byte closed framing and complete-source digest domain.
    let length = 60 + table.len() + column.len();
    let mut bytes = b"TCAC0004".to_vec();
    bytes.extend((length as u32).to_le_bytes());
    bytes.extend(4_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend((before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    bytes.extend(digest.finalize());
    bytes.extend((table.len() as u16).to_le_bytes());
    bytes.extend((column.len() as u16).to_le_bytes());
    bytes.extend(table.as_bytes());
    bytes.extend(column.as_bytes());
    bytes
}

fn decode(bytes: &[u8]) -> NativeRepeatedDefaultZeroAddColumnIntent {
    NativeRepeatedDefaultZeroAddColumnIntent::decode(bytes, &mut budget(), &|| false).unwrap()
}
