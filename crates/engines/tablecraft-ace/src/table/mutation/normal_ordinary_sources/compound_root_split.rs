//! Four frozen compound DATA/INDEX source views; no mutation or preview call.
use crate::{
    IndexCheckStatus, NativeImage, check_table_constraints, inspect_table_index_counter,
    inspect_table_properties, inspect_table_schema, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(223);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn u16_at(raw: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(raw[at..at + 2].try_into().unwrap()))
}

fn u32_at(raw: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(raw[at..at + 4].try_into().unwrap())
}

fn frame(raw: &[u8], page: usize, slot: usize) -> (usize, usize) {
    let at = page * 4096;
    assert_eq!(&raw[at..at + 2], &[1, 1]);
    assert!(slot < u16_at(raw, at + 12));
    let word = u16_at(raw, at + 14 + 2 * slot);
    assert_eq!(word & !8191, 0);
    let start = word;
    let end = if slot == 0 {
        4096
    } else {
        u16_at(raw, at + 12 + 2 * slot) & 8191
    };
    assert!(14 + 2 * u16_at(raw, at + 12) <= start && start < end && end <= 4096);
    (at + start, at + end)
}

fn payload(seed: usize) -> Vec<u8> {
    (0..84)
        .map(|position| (position * 31 + seed) as u8)
        .collect()
}

fn record(id: i32, bytes: &[u8]) -> Vec<u8> {
    assert_eq!(bytes.len(), 84);
    let mut raw = 2_u16.to_le_bytes().to_vec();
    raw.extend_from_slice(&id.to_le_bytes());
    raw.extend_from_slice(bytes);
    raw.extend_from_slice(&[90, 0, 6, 0, 1, 0, 3]);
    assert_eq!(raw.len(), 97);
    raw
}

fn source_rows() -> Vec<Vec<Value>> {
    (-226_i32..=225)
        .filter(|id| *id != 0)
        .enumerate()
        .map(|(ordinal, id)| vec![Value::Int32(id), Value::Binary(payload(ordinal))])
        .collect()
}

fn inline_cell(original: &[u8], reference: u32, page: u32) -> (usize, u8) {
    let (start, end) = frame(
        original,
        (reference >> 8) as usize,
        (reference & 255) as usize,
    );
    assert_eq!(end - start, 69);
    assert_eq!(original[start], 0);
    let base = u32_at(original, start + 1);
    let bit = usize::try_from(page.checked_sub(base).unwrap()).unwrap();
    assert!(bit < 512);
    (start + 5 + bit / 8, 1 << (bit % 8))
}

fn claim(after: &mut [u8], original: &[u8], cell: (usize, u8), before: bool, final_: bool) {
    let (offset, mask) = cell;
    assert_eq!(original[offset] & mask != 0, before);
    if final_ {
        after[offset] |= mask;
    } else {
        after[offset] &= !mask;
    }
}

fn literal(original: &[u8], td: u32, root: u32, pages: &[u32]) -> Vec<u8> {
    assert_eq!(pages.len(), 11);
    let mut keys = Vec::new();
    for (ordinal, id) in (-226_i32..=225).filter(|id| *id != 0).enumerate() {
        let page = pages[ordinal / 41];
        let slot = ordinal % 41;
        let at = page as usize * 4096;
        assert_eq!(u32_at(original, at + 4), td);
        assert_eq!(u16_at(original, at + 12), 41);
        assert_eq!(u16_at(original, at + 2), 23);
        assert!(u16_at(original, at + 2) < 97 + 2);
        let (start, end) = frame(original, page as usize, slot);
        assert_eq!(&original[start..end], record(id, &payload(ordinal)));
        keys.push(entry(id, (page << 8) | slot as u32));
    }

    let root_at = root as usize * 4096;
    assert_eq!(&original[root_at..root_at + 2], &[4, 1]);
    assert_eq!(u32_at(original, root_at + 4), td);
    assert_eq!(u16_at(original, root_at + 24), 1);
    assert_eq!(u16_at(original, root_at + 2), 7);
    let mut active = Vec::new();
    let mut bitmap = [0_u8; 453];
    for (position, key) in keys.iter().enumerate() {
        active.extend_from_slice(if position == 0 { key } else { &key[1..] });
        let end = active.len();
        bitmap[end / 8] |= 1 << (end % 8);
    }
    assert_eq!(active.len(), 3609);
    assert_eq!(&original[root_at + 27..root_at + 480], &bitmap);
    assert_eq!(&original[root_at + 480..root_at + 4089], active);

    let data = (original.len() / 4096) as u32;
    let right = data + 1;
    let left = data + 2;
    let mut after = original.to_vec();
    let mut new_data = vec![0; 4096];
    new_data[..2].copy_from_slice(&[1, 1]);
    new_data[2..4].copy_from_slice(&3983_u16.to_le_bytes());
    new_data[4..8].copy_from_slice(&td.to_le_bytes());
    new_data[12..14].copy_from_slice(&1_u16.to_le_bytes());
    new_data[14..16].copy_from_slice(&3999_u16.to_le_bytes());
    new_data[3999..].copy_from_slice(&record(226, &payload(226)));
    after.extend_from_slice(&new_data);
    keys.push(entry(226, data << 8));
    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    after.extend_from_slice(&fresh_leaf(td, &keys[226..], left, 0));
    after.extend_from_slice(&fresh_leaf(td, &keys[..226], 0, right));

    after[root_at] = 3;
    after[root_at + 2..root_at + 4].copy_from_slice(&3603_u16.to_le_bytes());
    after[root_at + 12..root_at + 20].fill(0);
    after[root_at + 20..root_at + 24].copy_from_slice(&right.to_le_bytes());
    after[root_at + 24..root_at + 26].fill(0);
    after[root_at + 26] = 1;
    after[root_at + 27..root_at + 480].fill(0);
    after[root_at + 28] = 32;
    after[root_at + 480..root_at + 489].copy_from_slice(&keys[225]);
    after[root_at + 489..root_at + 493].copy_from_slice(&left.to_be_bytes());

    let td_at = td as usize * 4096;
    for at in [td_at + 16, td_at + 67] {
        assert_eq!(u32_at(original, at), 451);
        after[at..at + 4].copy_from_slice(&452_u32.to_le_bytes());
    }

    let owner_ref = u32_at(original, td_at + 55);
    assert_eq!(owner_ref, (td + 1) << 8);
    let hint_ref = owner_ref | 1;
    let index_ref = owner_ref | 2;
    for (reference, expected_base) in [(256, 0), (hint_ref, 0), (index_ref, root)] {
        let (start, end) = frame(
            original,
            (reference >> 8) as usize,
            (reference & 255) as usize,
        );
        assert_eq!(end - start, 69);
        assert_eq!(original[start], 0);
        assert_eq!(u32_at(original, start + 1), expected_base);
    }
    let (owner_start, owner_end) = frame(original, (td + 1) as usize, 0);
    assert_eq!(owner_end - owner_start, 69);
    assert_eq!(original[owner_start], 1);
    let owner_bitmap = u32_at(original, owner_start + 1) as usize;
    assert_eq!(&original[owner_start + 5..owner_end], &[0; 64]);
    assert_eq!(
        &original[owner_bitmap * 4096..owner_bitmap * 4096 + 4],
        &[5, 1, 0, 0]
    );
    for page in 0..(original.len() / 4096 + 3) {
        let cell = owner_bitmap * 4096 + 4 + page / 8;
        assert_eq!(
            original[cell] & (1 << (page % 8)) != 0,
            pages.contains(&(page as u32))
        );
    }
    for page in 0..512_u32 {
        let (at, mask) = inline_cell(original, hint_ref, page);
        assert_eq!(original[at] & mask != 0, page == *pages.last().unwrap());
    }
    for page in root..root + 512 {
        let (at, mask) = inline_cell(original, index_ref, page);
        assert_eq!(original[at] & mask != 0, page == root);
    }

    // Qualify every role from the original bytes; shared-byte claims compose.
    for page in [data, right, left] {
        claim(
            &mut after,
            original,
            inline_cell(original, 256, page),
            true,
            false,
        );
        let owned_cell = (owner_bitmap * 4096 + 4 + page as usize / 8, 1 << (page % 8));
        claim(&mut after, original, owned_cell, false, page == data);
        claim(
            &mut after,
            original,
            inline_cell(original, hint_ref, page),
            false,
            page == data,
        );
        claim(
            &mut after,
            original,
            inline_cell(original, index_ref, page),
            false,
            page != data,
        );
    }
    claim(
        &mut after,
        original,
        inline_cell(original, hint_ref, *pages.last().unwrap()),
        true,
        false,
    );
    for page in pages {
        let start = *page as usize * 4096;
        assert_eq!(&after[start..start + 4096], &original[start..start + 4096]);
    }
    assert_eq!(
        &after[root_at + 493..root_at + 4096],
        &original[root_at + 493..root_at + 4096]
    );
    assert_eq!(after.len(), original.len() + 3 * 4096);
    after
}

fn entry(id: i32, reference: u32) -> Vec<u8> {
    let mut raw = vec![127];
    raw.extend_from_slice(&((id as u32) ^ 0x8000_0000).to_be_bytes());
    raw.extend_from_slice(&reference.to_be_bytes());
    raw
}

fn fresh_leaf(td: u32, entries: &[Vec<u8>], previous: u32, next: u32) -> Vec<u8> {
    assert_eq!(entries.len(), 226);
    let mut raw = vec![0; 4096];
    raw[..2].copy_from_slice(&[4, 1]);
    raw[2..4].copy_from_slice(&1582_u16.to_le_bytes());
    raw[4..8].copy_from_slice(&td.to_le_bytes());
    raw[12..16].copy_from_slice(&previous.to_le_bytes());
    raw[16..20].copy_from_slice(&next.to_le_bytes());
    for (position, item) in entries.iter().enumerate() {
        assert_eq!(item.len(), 9);
        let end = 9 * (position + 1);
        raw[480 + end - 9..480 + end].copy_from_slice(item);
        raw[27 + end / 8] |= 1 << (end % 8);
    }
    raw
}

fn inspect(raw: &[u8], td: u32, expected: &[Vec<Value>]) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let catalog = inspect_table_schema(
        &mut image.reader(),
        SCOPE,
        "MSysObjects",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(catalog.columns().len(), 17);
    let snapshot = read_table(
        &mut image.reader(),
        SCOPE,
        "BinaryStorage",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(snapshot.rows(), expected);
    assert_eq!(snapshot.schema().definition_page(), td);
    let fields = snapshot.schema().columns();
    assert_eq!(fields.len(), 2);
    assert_eq!(
        (fields[0].name(), fields[0].native_type(), fields[0].flags()),
        ("ID", 4, 3)
    );
    assert_eq!(
        (
            fields[1].name(),
            fields[1].native_type(),
            fields[1].storage_length()
        ),
        ("Bytes", 9, 255)
    );
    let properties = inspect_table_properties(
        &mut image.reader(),
        SCOPE,
        "BinaryStorage",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(!properties.raw_bytes().is_empty());
    let group = properties
        .groups()
        .iter()
        .find(|group| group.name() == "ID")
        .unwrap();
    assert!(group.get("Required").unwrap().boolean().unwrap());
    let constraints = check_table_constraints(
        &mut image.reader(),
        SCOPE,
        "BinaryStorage",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), expected.len());
    assert!(constraints.relationships().is_empty());
    let checked = crate::table::indexes::check_table_retained(
        &mut image.reader(),
        SCOPE,
        "BinaryStorage",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), expected.len());
    assert_eq!(checked.indexes().len(), 1);
    assert_eq!(checked.indexes()[0].status(), IndexCheckStatus::Checked);
    let index = checked.indexes()[0].index();
    assert_eq!(index.name(), "PrimaryID");
    assert_eq!(index.root_page(), td + 2);
    assert_eq!(index.usage_map(), ((td + 1) << 8) | 2);
    let flags = index.constraints().unwrap();
    assert!(flags.primary_key() && flags.unique() && flags.required());
    assert!(!flags.ignore_nulls());
    let rows = read_table_index(
        &mut image.reader(),
        SCOPE,
        "BinaryStorage",
        "PrimaryID",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows.rows(), expected);
    let counter = inspect_table_index_counter(
        &mut image.reader(),
        SCOPE,
        "BinaryStorage",
        "PrimaryID",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(counter.value() as usize, expected.len());

    // Exact current normal action Source and NoRequired conjunction. A first
    // refusal is a diagnostic, not permission to strip or retune the source.
    let source =
        super::super::normal_actions::capture(&image, "BinaryStorage", &mut budget(), &|| false)
            .expect("current full Source/NoRequired root-split source qualification");
    assert_eq!(source.records.len(), expected.len());
    assert_eq!(source.table.rows(), expected);
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires four frozen full84 originals/models; read-only source qualification"]
fn existing_compound_data_and_index_root_split_views_have_complete_source_coverage() {
    let repository = PathBuf::from(std::env::var_os("TABLECRAFT_ROOT_SPLIT_REPOSITORY").unwrap());
    assert!(repository.is_absolute());
    for (
        before_path,
        before_hash,
        before_bytes,
        after_path,
        after_hash,
        after_bytes,
        td,
        root,
        pages,
    ) in [
        (
            ".cache/ace16-insert-root-split-reference-first-20261027/V2016-full84-source.accdb",
            "69a6dff12414839a8440be4ccc86d43658f14f1315ae0f4461fd236794295d9e",
            425984,
            ".cache/ace16-insert-root-split-independent-next-20261027/expected/V2016-full84.expected.bin",
            "5d3b8b2f5cb5ca065f5837aec5197c7389e7b200e55c005be27c2dcc105c6787",
            438272,
            89_u32,
            91_u32,
            [92_u32, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103],
        ),
        (
            ".cache/ace16-insert-root-split-reference-first-20261027/V2019-full84-source.accdb",
            "f695ebe4a18c392c014f49506cd741bac7760bd4b398f83fb4c02925275e2e38",
            499712,
            ".cache/ace16-insert-root-split-independent-next-20261027/expected/V2019-full84.expected.bin",
            "4597231d353a5e669a8ea029d8caeb44a43ffa5e3628e91593f26fcc82991752",
            512000,
            106_u32,
            108_u32,
            [110_u32, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121],
        ),
    ] {
        let before = fs::read(repository.join(before_path)).unwrap();
        let after = fs::read(repository.join(after_path)).unwrap();
        assert_eq!(
            (before.len(), hash(&before)),
            (before_bytes, before_hash.to_owned())
        );
        assert_eq!(
            (after.len(), hash(&after)),
            (after_bytes, after_hash.to_owned())
        );
        assert_eq!(literal(&before, td, root, &pages), after);
        let original = source_rows();
        inspect(&before, td, &original);
        let mut rows = original;
        rows.push(vec![Value::Int32(226), Value::Binary(payload(226))]);
        inspect(&after, td, &rows);
    }
}
