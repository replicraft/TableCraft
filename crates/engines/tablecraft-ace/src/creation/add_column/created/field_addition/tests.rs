//! Independent complete images and explicit frozen-profile/source refusals.
mod dictionary;
mod prefix;
mod refusal;
mod sdk;

use super::{FieldAdditionCandidate, FieldAdditionTarget};
use crate::NativeImage;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::mixed as raw, support::bytes::hex,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 200_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(901),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn hash(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn text(units: usize, compressed: bool) -> FieldAdditionTarget {
    FieldAdditionTarget::ShortText {
        max_utf16_units: units,
        unicode_compression: compressed,
    }
}

fn qualify(
    before: &[u8],
    expected: &[u8],
    column: &str,
    target: FieldAdditionTarget,
    sdk_hash: &str,
) -> FieldAdditionCandidate {
    assert_eq!(hash(expected), sdk_hash, "independent complete SDK oracle");
    let untouched = before.to_vec();
    image(before)
        .check_created_field_addition_source("Table1", &mut budget(), &|| false)
        .unwrap();
    let candidate = image(before)
        .prepare_created_add_field_candidate("Table1", column, target, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(before, untouched);
    assert_eq!(candidate.before_bytes(), before);
    assert_eq!(candidate.after_bytes(), expected);
    assert_eq!(candidate.table(), "Table1");
    assert_eq!(candidate.column(), column);
    assert_eq!(candidate.target(), target);
    candidate
}
