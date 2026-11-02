//! Independent literal same-owned-page forwarding source reads, no writer API.
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
#[ignore = "requires explicit genuine originals and authored alternate-owned source directory"]
fn genuine_alternate_preowned_data_sources_and_growth_read_before_any_writer() {
    let originals =
        PathBuf::from(std::env::var_os("TABLECRAFT_PROPERTY_LVAL_ORIGINALS_DIR").unwrap());
    let sources =
        PathBuf::from(std::env::var_os("TABLECRAFT_ALTERNATE_OWNED_EXPECTED_DIR").unwrap());
    assert!(originals.is_absolute() && sources.is_absolute());
    let mut views = 0;
    for (header, digest, definition, maps, bitmap, old, new) in [
        (
            5_u8,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            107_u32,
            108_u32,
            111_usize,
            110_u32,
            112_u32,
        ),
        (
            6_u8,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            125_u32,
            126_u32,
            128_usize,
            127_u32,
            129_u32,
        ),
    ] {
        let path = originals.join(format!("h{header}.accdb"));
        let original = fs::read(&path).unwrap();
        assert_eq!(hash(&original), digest);
        assert_eq!(original.len(), new as usize * 4096);
        let base = old as usize * 4096;
        let mut before = original.clone();
        let mut empty = vec![0; 4096];
        empty[..4].copy_from_slice(&[1, 1, 242, 15]);
        empty[4..8].copy_from_slice(&definition.to_le_bytes());
        before.extend_from_slice(&empty);
        before[base + 2..base + 4].copy_from_slice(&[178, 13]);
        before[base + 12..base + 14].copy_from_slice(&[255, 0]);
        for slot in 1..255 {
            let at = base + 14 + slot * 2;
            before[at..at + 2].copy_from_slice(&[190, 143]);
        }
        let owned = bitmap * 4096 + 4 + new as usize / 8;
        before[owned] |= 1 << (new % 8);
        let candidate = maps as usize * 4096 + 3958 + 5;
        before[candidate + old as usize / 8] &= !(1 << (old % 8));
        before[candidate + new as usize / 8] |= 1 << (new % 8);
        let global = 4096 + 4027 + 5 + new as usize / 8;
        before[global] &= !(1 << (new % 8));
        let source =
            fs::read(sources.join(format!("h{header}-alternate-owned-before.accdb"))).unwrap();
        assert_eq!(source, before);
        read_view(
            SCOPE,
            budget,
            &source,
            definition,
            maps,
            Some("unchanged sentinel"),
        );
        memberships(&source, maps, old, new);
        let row = &original[base + 4030..base + 4096];
        let mut terminal = row[..6].to_vec();
        for _ in 0..19 {
            terminal.extend_from_slice(&[97, 0]);
        }
        terminal.extend_from_slice(&row[42..57]);
        terminal.extend_from_slice(&[59, 0, 44, 0, 6, 0, 2, 0, 7]);
        assert_eq!(terminal.len(), 68);
        let next = new as usize * 4096;
        let mut literal = source.clone();
        literal[next + 4028..next + 4096].copy_from_slice(&terminal);
        literal[next + 2..next + 4].copy_from_slice(&[172, 15]);
        literal[next + 12..next + 16].copy_from_slice(&[1, 0, 188, 143]);
        literal[base + 14..base + 16].copy_from_slice(&[190, 79]);
        literal[base + 4030..base + 4096].fill(0);
        literal[base + 4030..base + 4034].copy_from_slice(&(new << 8).to_le_bytes());
        let after =
            fs::read(sources.join(format!("h{header}-alternate-owned-growth.accdb"))).unwrap();
        assert_eq!(after, literal);
        assert_eq!(after.len(), source.len());
        read_view(
            SCOPE,
            budget,
            &after,
            definition,
            maps,
            Some("aaaaaaaaaaaaaaaaaaa"),
        );
        memberships(&after, maps, old, new);
        assert_eq!(fs::read(path).unwrap(), original);
        views += 2;
    }
    assert_eq!(views, 4);
}
