//! Explicit original/expected reads only; no mutation or permission API is called.
use tablecraft_testkit::support::ace::sentinel::read_view;

use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(133);
const OLD: &str = "unchanged sentinel";
const RECIPES: [(&str, Option<&str>); 6] = [
    ("same", Some("qualified sentinel")),
    ("shorter", Some("changed sentinel")),
    ("empty", Some("")),
    ("clear", None),
    ("noop", Some(OLD)),
    ("empty-group", Some(OLD)),
];
const SOURCES: [(u8, usize, &str, u32, u32, u32); 2] = [
    (
        5,
        458752,
        "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
        107,
        108,
        110,
    ),
    (
        6,
        528384,
        "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
        125,
        126,
        127,
    ),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn literal(before: &[u8], data: u32, marker: Option<&str>) -> Vec<u8> {
    let mut after = before.to_vec();
    let start = data as usize * PAGE + 4030;
    let original = &before[start..start + 66];
    let original_marker = OLD
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(&original[6..42], original_marker.as_slice());
    assert_eq!(
        &original[42..57],
        &[3, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 199, 0, 255]
    );
    let replacement = marker
        .unwrap_or("")
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    assert!(replacement.len() <= 36);
    let row = &mut after[start..start + 66];
    row[6..6 + replacement.len()].copy_from_slice(&replacement);
    row[6 + replacement.len()..21 + replacement.len()].copy_from_slice(&original[42..57]);
    for (at, offset) in [
        (57, 21 + replacement.len()),
        (59, 6 + replacement.len()),
        (61, 6),
    ] {
        row[at..at + 2].copy_from_slice(&(offset as u16).to_le_bytes());
    }
    row[65] = (original[65] & !2) | if marker.is_some() { 2 } else { 0 };
    after
}

#[test]
#[ignore = "requires explicit original and independently constructed expected directories"]
fn genuine_marker_property_and_readonly_ole_views_qualify_before_any_writer() {
    let originals =
        PathBuf::from(std::env::var_os("TABLECRAFT_PROPERTY_LVAL_ORIGINALS_DIR").unwrap());
    let expected =
        PathBuf::from(std::env::var_os("TABLECRAFT_PROPERTY_LVAL_EXPECTED_DIR").unwrap());
    assert!(originals.is_absolute() && expected.is_absolute());
    let mut views = 0;
    for (header, length, digest, definition, maps, data) in SOURCES {
        let before = fs::read(originals.join(format!("h{header}.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(hash(&before), digest);
        read_view(SCOPE, budget, &before, definition, maps, Some(OLD));
        views += 1;
        for (recipe, marker) in RECIPES {
            let after =
                fs::read(expected.join(format!("h{header}-marker-{recipe}.accdb"))).unwrap();
            assert_eq!(after, literal(&before, data, marker));
            read_view(SCOPE, budget, &after, definition, maps, marker);
            views += 1;
        }
        assert_eq!(
            fs::read(originals.join(format!("h{header}.accdb"))).unwrap(),
            before
        );
    }
    assert_eq!(views, 14);
}
