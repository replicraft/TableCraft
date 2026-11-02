//! Independent literal same-owned-page forwarding source reads, no writer API.
use tablecraft_testkit::support::ace::sentinel::read_view;

use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(141);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
#[ignore = "requires independently constructed first and second forwarded source images"]
fn genuine_forwarded_second_edit_sources_read_before_any_writer() {
    let first = PathBuf::from(std::env::var_os("TABLECRAFT_OWNED_GROWTH_EXPECTED_DIR").unwrap());
    let second =
        PathBuf::from(std::env::var_os("TABLECRAFT_OWNED_GROWTH_SECOND_EXPECTED_DIR").unwrap());
    assert!(first.is_absolute() && second.is_absolute());
    let mut views = 0;
    for (header, digest, definition, maps, data) in [
        (
            5_u8,
            "2bfdbfc9caddaa5151bbd3006f095e49c4afeba02d0d5ac9b4ffeb84439ce17e",
            107,
            108,
            110_usize,
        ),
        (
            6_u8,
            "4cd5c2f9ca8c65e2cb74ec0269712adb8662b0457af6ecce121743dcec1a5775",
            125,
            126,
            127_usize,
        ),
    ] {
        let first_path = first.join(format!("h{header}-marker-owned-growth.accdb"));
        let before = fs::read(&first_path).unwrap();
        assert_eq!(hash(&before), digest);
        read_view(
            SCOPE,
            budget,
            &before,
            definition,
            maps,
            Some("aaaaaaaaaaaaaaaaaaa"),
        );
        let after =
            fs::read(second.join(format!("h{header}-marker-forwarded-second.accdb"))).unwrap();
        let start = data * 4096 + 3968;
        let end = start + 38;
        assert_eq!(
            &before[start..end],
            b"a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0"
        );
        let mut literal = before.clone();
        for at in (start..end).step_by(2) {
            literal[at] = b'b';
        }
        assert_eq!(after, literal);
        assert_eq!(&after[..start], &before[..start]);
        assert_eq!(&after[end..], &before[end..]);
        read_view(
            SCOPE,
            budget,
            &after,
            definition,
            maps,
            Some("bbbbbbbbbbbbbbbbbbb"),
        );
        assert_eq!(fs::read(first_path).unwrap(), before);
        views += 2;
    }
    assert_eq!(views, 4);
}
