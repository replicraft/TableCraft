//! Independent wire construction and complete raw AFTERs, without private files.
mod codec;
mod native;
mod recovery;

use super::*;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_io::journal::{self, JournalError, RecoveryOutcome, V2State, inspect_v2};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::mixed::after_off::{
        self as raw, history as sources,
    },
    support::io::journal::MemoryStorage,
};

const SCOPE: CatalogScope = CatalogScope::new(1200);
const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn text() -> FieldAdditionTarget {
    FieldAdditionTarget::ShortText {
        max_utf16_units: 255,
        unicode_compression: true,
    }
}

fn plan(
    before: &[u8],
    column: &str,
    target: FieldAdditionTarget,
) -> RecoverableFieldAdditionAfterOffHistoryPlan {
    image(before)
        .plan_recoverable_created_field_addition_after_off_history(
            "Table1",
            column,
            target,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

fn literal(before: &[u8], table: &str, column: &str, target: FieldAdditionTarget) -> Vec<u8> {
    // Independent literal 64-byte frame, including fixed target policy tags.
    // No production format offsets, tags or binding implementation enter this oracle.
    let (tag, width, compression) = match target {
        FieldAdditionTarget::NumberLongZero => (1_u16, 0_u16, 0_u16),
        FieldAdditionTarget::ShortText {
            max_utf16_units,
            unicode_compression,
        } => (2, max_utf16_units as u16, u16::from(unicode_compression)),
    };
    let mut bytes = b"TCAC0007".to_vec();
    bytes.extend(((64 + table.len() + column.len()) as u32).to_le_bytes());
    bytes.extend(7_u16.to_le_bytes());
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

fn decode(bytes: &[u8]) -> NativeFieldAdditionAfterOffHistoryIntent {
    NativeFieldAdditionAfterOffHistoryIntent::decode(bytes, &mut budget(), &|| false).unwrap()
}
