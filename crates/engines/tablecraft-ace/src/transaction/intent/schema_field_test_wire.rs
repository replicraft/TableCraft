//! Independent literal schema request framing shared by DROP and RENAME tests.
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;

#[derive(Clone, Copy)]
pub(super) struct Facts {
    pub definition: u32,
    pub number: u16,
    pub column_id: u16,
}

pub(super) fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 2_000_000_000)
}

/// First-party sources bind native catalog ID to the definition. Decode their
/// literal last descriptor directly, without asking the native candidate.
#[cfg(feature = "ace16-create")]
pub(super) fn tail(before: &[u8], definition: u32) -> Facts {
    let page = definition as usize * 4096;
    let count = u16::from_le_bytes(before[page + 45..page + 47].try_into().unwrap());
    let field = page + 75 + 25 * usize::from(count - 1);
    Facts {
        definition,
        number: u16::from_le_bytes(before[field + 5..field + 7].try_into().unwrap()),
        column_id: u16::from_le_bytes(before[field + 9..field + 11].try_into().unwrap()),
    }
}

fn prefix(before: &[u8], magic: &[u8; 8], profile: u16, size: usize, facts: Facts) -> Vec<u8> {
    let mut bytes = magic.to_vec();
    bytes.extend((size as u32).to_le_bytes());
    bytes.extend(profile.to_le_bytes());
    bytes.extend(0_u16.to_le_bytes());
    bytes.extend((before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    bytes.extend(digest.finalize());
    bytes.extend((facts.definition as i32).to_le_bytes());
    bytes.extend(facts.definition.to_le_bytes());
    bytes.extend(facts.number.to_le_bytes());
    bytes.extend(facts.column_id.to_le_bytes());
    bytes
}

pub(super) fn drop(before: &[u8], facts: Facts, column: &str) -> Vec<u8> {
    let mut bytes = prefix(before, b"TCFD0003", 3, 76 + 6 + column.len(), facts);
    bytes.extend(6_u16.to_le_bytes());
    bytes.extend((column.len() as u16).to_le_bytes());
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(b"Table1");
    bytes.extend(column.as_bytes());
    bytes
}

pub(super) fn rename(before: &[u8], facts: Facts, column: &str, new_name: &str) -> Vec<u8> {
    let mut bytes = prefix(
        before,
        b"TCFR0006",
        6,
        80 + 6 + column.len() + new_name.len(),
        facts,
    );
    bytes.extend((column.len() as u16).to_le_bytes());
    bytes.extend(6_u16.to_le_bytes());
    bytes.extend((column.len() as u16).to_le_bytes());
    bytes.extend((new_name.len() as u16).to_le_bytes());
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(b"Table1");
    bytes.extend(column.as_bytes());
    bytes.extend(new_name.as_bytes());
    bytes
}
