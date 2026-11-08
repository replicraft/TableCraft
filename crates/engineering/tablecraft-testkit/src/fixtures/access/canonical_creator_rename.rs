//! Independent options-then-rename bytes; no native planner or record encoder.
use super::{
    canonical_creation, canonical_creation_primary, canonical_creation_properties,
    canonical_database_options, canonical_native17_creation, put16, wide_properties,
};

const PAGE: usize = 4096;

pub struct Pair {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

/// Base creator carriers and closed metadata examples, all independently built.
pub fn originals() -> Vec<Vec<u8>> {
    let mut originals = Vec::new();
    for case in [
        canonical_creation::Case::Empty,
        canonical_creation::Case::One,
    ] {
        originals.push(canonical_native17_creation::source(
            canonical_creation::source(case).bytes,
        ));
    }
    for case in [
        canonical_creation_primary::Case::Empty,
        canonical_creation_primary::Case::One,
    ] {
        let manual = canonical_creation_primary::source(case).bytes;
        let mut generated = manual.clone();
        assert_eq!(generated[7 * PAGE + 75 + 15], 3);
        generated[7 * PAGE + 75 + 15] = 7;
        originals.push(manual);
        originals.push(generated);
    }
    let ordinary = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::One).bytes,
    );
    originals.push(canonical_creation_properties::with_default_expressions(
        ordinary.clone(),
        &[("Score", "0")],
    ));
    originals.push(
        canonical_creation_properties::source(
            canonical_creation_properties::Case::RequiredEmptyAllowed,
        )
        .bytes,
    );
    originals.push(with_text_property(
        ordinary,
        "Score",
        "Description",
        10,
        "Inert [Items].Score and Now()",
    ));
    originals
}

/// One independently framed inert text property, including refusal hypotheses.
pub fn with_text_property(
    mut creator: Vec<u8>,
    column: &str,
    key: &str,
    kind: u8,
    text: &str,
) -> Vec<u8> {
    let label = column
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let value = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut body = ((label.len() + 6) as u32).to_le_bytes().to_vec();
    body.extend_from_slice(&(label.len() as u16).to_le_bytes());
    body.extend_from_slice(&label);
    body.extend_from_slice(&((value.len() + 8) as u16).to_le_bytes());
    body.extend_from_slice(&[1, kind]);
    body.extend_from_slice(&0_u16.to_le_bytes());
    body.extend_from_slice(&(value.len() as u16).to_le_bytes());
    body.extend_from_slice(&value);
    let properties = wide_properties::bag(&[
        wide_properties::names(&[key]),
        wide_properties::block(1, &body),
    ]);
    canonical_creation_properties::attach_properties(&mut creator, &properties);
    creator
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

/// The raw creator original has Items at slot7; explicit OFF appends slot8.
/// Rebuild Name/footer and physical row placement from independent native17 facts.
pub fn pair(creator: &[u8], new_name: &str) -> Pair {
    assert!(!new_name.is_empty() && new_name.encode_utf16().count() <= 64);
    let before = canonical_database_options::explicit_off(creator);
    let page = &before[4 * PAGE..5 * PAGE];
    assert_eq!(word(page, 12), 9);
    let user_end = word(page, 14 + 2 * 6);
    let user_start = word(page, 14 + 2 * 7);
    let database_start = word(page, 14 + 2 * 8);
    let original = &page[user_start..user_end];
    let database = &page[database_start..user_start];
    assert_eq!(word(original, 0), 17);
    assert_eq!(word(original, original.len() - 5), 11);
    assert_eq!(word(original, original.len() - 7), 32);
    let old_name_end = word(original, original.len() - 9);
    assert_eq!(&original[32..old_name_end], b"I\0t\0e\0m\0s\0");
    let mut name = Vec::new();
    if new_name.starts_with('\u{feff}') {
        name.extend_from_slice(&[0xff, 0xfe, 0]);
    }
    name.extend(new_name.encode_utf16().flat_map(u16::to_le_bytes));
    let footer = original.len() - 29;
    assert_eq!(word(original, footer), footer);
    let mut row = original[..32].to_vec();
    row.extend_from_slice(&name);
    row.extend_from_slice(&original[old_name_end..footer]);
    let end = row.len();
    row.extend_from_slice(&(end as u16).to_le_bytes());
    for variable in (0..11).rev() {
        let old = word(original, original.len() - 7 - 2 * variable);
        let offset = if variable == 0 {
            32
        } else {
            old - (old_name_end - 32) + name.len()
        };
        row.extend_from_slice(&(offset as u16).to_le_bytes());
    }
    row.extend_from_slice(&11_u16.to_le_bytes());
    row.extend_from_slice(&original[original.len() - 3..]);

    let new_user = user_end.checked_sub(row.len()).unwrap();
    let new_database = new_user.checked_sub(database.len()).unwrap();
    assert!(new_database >= 32);
    let mut after = before.clone();
    let page = &mut after[4 * PAGE..5 * PAGE];
    page[new_user..user_end].copy_from_slice(&row);
    page[new_database..new_user].copy_from_slice(database);
    put16(page, 14 + 2 * 7, new_user as u16);
    put16(page, 14 + 2 * 8, new_database as u16);
    put16(page, 2, (new_database - 32) as u16);
    Pair { before, after }
}
