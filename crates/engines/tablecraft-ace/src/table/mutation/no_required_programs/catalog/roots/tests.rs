//! Pinned genuine catalog roots, without any planner or writer calls.
use super::capture;
use crate::{
    EditError, Header, NativeImage,
    format::{allocation, reader::Reader},
    inspect_table_schema,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(133);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn facts(original: &[u8]) -> (Header, Vec<u32>) {
    let image =
        NativeImage::capture(&mut Cursor::new(original), SCOPE, &mut budget(), &|| false).unwrap();
    let schema = inspect_table_schema(
        &mut image.reader(),
        SCOPE,
        "MSysObjects",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut bytes = image.reader();
    let mut allowance = budget();
    let mut reader = Reader {
        source: &mut bytes,
        header: image.catalog().header(),
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    let owned = allocation::pages(&mut reader, schema.layout.usage).unwrap();
    (image.catalog().header(), owned)
}

fn read(original: &[u8], bytes: &[u8]) -> Result<Vec<super::LogicalRow>, EditError> {
    let (header, owned) = facts(original);
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    let mut reader = Reader {
        source: &mut source,
        header,
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    capture(&mut reader, &owned, 2)
}

#[test]
#[ignore = "requires the two hash-pinned genuine Sentinel originals"]
fn genuine_forwarding_counts_logical_roots_once_and_retains_hidden_body_identity() {
    for source in crate::properties::framing::tests::originals() {
        let rows = read(&source.bytes, &source.bytes).unwrap();
        let (count, page, slot, terminal, start, native) = if source.bytes[20] == 5 {
            (37, 17, 37, 109, 3948, 107_i32)
        } else {
            (41, 114, 2, 114, 3679, 125_i32)
        };
        assert_eq!(rows.len(), count);
        let selected = rows
            .iter()
            .find(|row| row.page == page && row.slot == slot)
            .unwrap();
        assert_eq!(selected.physical.offset, terminal * PAGE + start);
        assert_eq!(selected.physical.data.len(), 148);
        assert_eq!(&selected.physical.data[..2], &[17, 0]);
        assert_eq!(&selected.physical.data[2..6], &native.to_le_bytes());
        assert_eq!(&selected.physical.data[145..], &[0xff, 0x40, 0]);
        assert_eq!(selected.physical.offset + 62, source.property_offset);
        let hidden_slot = if source.bytes[20] == 5 { 0 } else { 3 };
        assert!(
            !rows
                .iter()
                .any(|row| row.page == terminal as u32 && row.slot == hidden_slot)
        );
    }
}

#[test]
#[ignore = "requires the two hash-pinned genuine Sentinel originals"]
fn unknown_flags_orphan_hidden_shared_terminal_and_cycle_refuse_before_catalog_semantics() {
    for source in crate::properties::framing::tests::originals() {
        let (root_page, root_slot, root_start, prior_start, prior_hidden_slot, target) =
            if source.bytes[20] == 5 {
                (17, 37, 164, 344, 36, 0x6d00_u32)
            } else {
                (114, 2, 3827, 4007, 1, 0x7203_u32)
            };
        let directory = root_page * PAGE + 14 + 2 * root_slot;
        let word = u16::from_le_bytes(source.bytes[directory..directory + 2].try_into().unwrap());
        let mut unknown = source.bytes.clone();
        unknown[directory..directory + 2].copy_from_slice(&(word | 0x2000).to_le_bytes());
        assert!(read(&source.bytes, &unknown).is_err());

        let mut orphan = source.bytes.clone();
        orphan[directory..directory + 2].copy_from_slice(&(word | 0x8000).to_le_bytes());
        assert!(read(&source.bytes, &orphan).is_err());

        let mut shared = source.bytes.clone();
        shared[root_page * PAGE + prior_start..root_page * PAGE + prior_start + 4]
            .copy_from_slice(&target.to_le_bytes());
        // Make the former prior hidden body an empty deleted slot. Its stale
        // bytes become retained root capacity, so an orphan guard cannot mask
        // loss of the unique-terminal-owner guard in this focused layer.
        let prior_word = root_page * PAGE + 14 + 2 * prior_hidden_slot;
        shared[prior_word..prior_word + 2]
            .copy_from_slice(&(0x8000_u16 | prior_start as u16).to_le_bytes());
        assert!(read(&source.bytes, &shared).is_err());

        let mut cycle = source.bytes.clone();
        let root = ((root_page as u32) << 8) | root_slot as u32;
        cycle[root_page * PAGE + root_start..root_page * PAGE + root_start + 4]
            .copy_from_slice(&root.to_le_bytes());
        assert!(read(&source.bytes, &cycle).is_err());
    }
}

#[test]
#[ignore = "requires the hash-pinned genuine header5 Sentinel original"]
fn scoped_reader_rejects_unowned_intermediate_even_when_root_and_terminal_are_owned() {
    let sources = crate::properties::framing::tests::originals();
    let source = &sources[0];
    assert_eq!(source.bytes[20], 5);
    let (header, mut owned) = facts(&source.bytes);
    assert!(owned.contains(&17) && owned.contains(&109));
    owned.retain(|number| *number != 109);
    let mut changed = source.bytes.clone();
    // Root17:37 -> intermediate109:0 -> terminal17:36. Only the intermediate
    // is outside the narrowed scope. All three native owner fields remain2.
    let hidden_word = 109 * PAGE + 14;
    changed[hidden_word..hidden_word + 2].copy_from_slice(&0xcf6c_u16.to_le_bytes());
    changed[109 * PAGE + 3948..109 * PAGE + 3952].copy_from_slice(&0x1124_u32.to_le_bytes());
    let page: &[u8; PAGE] = changed[17 * PAGE..18 * PAGE].try_into().unwrap();
    let mut bytes = Cursor::new(&changed);
    let mut allowance = budget();
    let mut reader = Reader {
        source: &mut bytes,
        header,
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    let mut visited = Vec::new();
    let result = reader.row_location_scoped(17, 37, 2, page, &owned, &mut |reference, _| {
        visited.push(reference);
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(visited, [0x1125]);
}
