//! Independent original requests and raw complete images qualify checked V2.
mod codec;
mod native;
mod recovery;

use super::*;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, V2State, inspect_v2};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::mixed as raw, support::io::journal::MemoryStorage,
};

const SCOPE: CatalogScope = CatalogScope::new(946);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn text(width: usize, compression: bool) -> FieldAdditionTarget {
    FieldAdditionTarget::ShortText {
        max_utf16_units: width,
        unicode_compression: compression,
    }
}

fn plan(before: &[u8], column: &str, target: FieldAdditionTarget) -> RecoverableFieldAdditionPlan {
    image(before)
        .plan_recoverable_created_field_addition("Table1", column, target, &mut budget(), &|| false)
        .unwrap()
}

fn literal(before: &[u8], table: &str, column: &str, target: FieldAdditionTarget) -> Vec<u8> {
    // Independent 64-byte framing; none of the production codec constants enter.
    let (tag, width, compression) = match target {
        FieldAdditionTarget::NumberLongZero => (1_u16, 0_u16, 0_u16),
        FieldAdditionTarget::ShortText {
            max_utf16_units,
            unicode_compression,
        } => (2, max_utf16_units as u16, u16::from(unicode_compression)),
    };
    let length = 64 + table.len() + column.len();
    let mut bytes = b"TCAC0005".to_vec();
    bytes.extend((length as u32).to_le_bytes());
    bytes.extend(5_u16.to_le_bytes());
    bytes.extend(tag.to_le_bytes());
    bytes.extend(width.to_le_bytes());
    bytes.extend(compression.to_le_bytes());
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

fn decode(bytes: &[u8]) -> NativeFieldAdditionIntent {
    NativeFieldAdditionIntent::decode(bytes, &mut budget(), &|| false).unwrap()
}
