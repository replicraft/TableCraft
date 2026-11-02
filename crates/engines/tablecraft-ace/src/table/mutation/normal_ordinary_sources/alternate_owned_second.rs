//! Independent cross-page forwarded second-edit source reads, no writer API.
use crate::NativeImage;
use crate::format::{allocation, reader::Reader};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::support::ace::sentinel::read_view;

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

fn memberships(raw: &[u8], maps: u32, old: u32, new: u32) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let mut bytes = image.reader();
    let mut work = budget();
    let mut reader = Reader {
        source: &mut bytes,
        header: image.catalog().header(),
        budget: &mut work,
        cancelled: &|| false,
        reads: 0,
    };
    assert_eq!(
        allocation::pages(&mut reader, maps << 8).unwrap(),
        [old, new]
    );
    assert_eq!(
        allocation::pages(&mut reader, (maps << 8) | 1).unwrap(),
        [new]
    );
    let global = allocation::RepresentedMap::read(
        &mut reader,
        allocation::GLOBAL_FREE_REFERENCE,
        allocation::MapRole::GlobalFree,
    )
    .unwrap();
    global.require_allocated(&mut reader, old).unwrap();
    global.require_allocated(&mut reader, new).unwrap();
}

#[test]
#[ignore = "requires qualified alternate-owned first and separately authored second images"]
fn alternate_owned_second_sources_read_before_any_writer() {
    let first = PathBuf::from(std::env::var_os("TABLECRAFT_ALTERNATE_OWNED_EXPECTED_DIR").unwrap());
    let second =
        PathBuf::from(std::env::var_os("TABLECRAFT_ALTERNATE_OWNED_SECOND_EXPECTED_DIR").unwrap());
    assert!(first.is_absolute() && second.is_absolute());
    let mut views = 0;
    for (header, digest, definition, maps, old, new) in [
        (
            5_u8,
            "b08e1cb05999cc30970ea97d8ea4b1bbc105ccb6cd05ccda106618914591645e",
            107,
            108,
            110,
            112,
        ),
        (
            6_u8,
            "28ce5406b7fa8b386c4ad6a1060e405c3b7c784d882d0df32f498b1d4b7c97c3",
            125,
            126,
            127,
            129,
        ),
    ] {
        let path = first.join(format!("h{header}-alternate-owned-growth.accdb"));
        let before = fs::read(&path).unwrap();
        assert_eq!(hash(&before), digest);
        read_view(
            SCOPE,
            budget,
            &before,
            definition,
            maps,
            Some("aaaaaaaaaaaaaaaaaaa"),
        );
        memberships(&before, maps, old, new);
        let start = new as usize * 4096 + 4034;
        let end = start + 38;
        for pair in before[start..end].as_chunks::<2>().0 {
            assert_eq!(pair, &[97, 0]);
        }
        let mut literal = before.clone();
        for at in (start..end).step_by(2) {
            literal[at] = 98;
        }
        let after =
            fs::read(second.join(format!("h{header}-alternate-owned-second.accdb"))).unwrap();
        assert_eq!(after, literal);
        read_view(
            SCOPE,
            budget,
            &after,
            definition,
            maps,
            Some("bbbbbbbbbbbbbbbbbbb"),
        );
        memberships(&after, maps, old, new);
        assert_eq!(fs::read(path).unwrap(), before);
        views += 2;
    }
    assert_eq!(views, 4);
}
