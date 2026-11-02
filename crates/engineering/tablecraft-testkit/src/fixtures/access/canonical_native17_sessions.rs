//! Independent two-field normal-session images, authored before creator calls.
use super::{canonical_creation, canonical_native17_creation};

pub struct Source {
    pub before: Vec<u8>,
    pub inserted: Vec<u8>,
    pub deleted: Vec<u8>,
}

pub fn source() -> Source {
    let old = canonical_creation::source(canonical_creation::Case::One);
    let before = canonical_native17_creation::source(old.bytes);
    assert_eq!(before.len(), 36864);
    let definition = 7 * 4096;
    let data = 8 * 4096;
    assert_eq!(
        &before[definition + 16..definition + 20],
        &1_u32.to_le_bytes()
    );
    assert_eq!(&before[data + 12..data + 16], &[1, 0, 0xf5, 0x0f]);
    assert_eq!(&before[data + 2..data + 4], &4069_u16.to_le_bytes());
    let mut inserted = before.clone();
    inserted[definition + 16..definition + 20].copy_from_slice(&2_u32.to_le_bytes());
    inserted[data + 2..data + 4].copy_from_slice(&4056_u16.to_le_bytes());
    inserted[data + 12..data + 14].copy_from_slice(&2_u16.to_le_bytes());
    inserted[data + 16..data + 18].copy_from_slice(&4074_u16.to_le_bytes());
    inserted[data + 4074..data + 4085].copy_from_slice(&[2, 0, 2, 0, 0, 0, 20, 0, 0, 0, 3]);
    let mut deleted = inserted.clone();
    deleted[definition + 16..definition + 20].copy_from_slice(&1_u32.to_le_bytes());
    deleted[data + 16..data + 18].copy_from_slice(&0xcfea_u16.to_le_bytes());
    assert_eq!(
        &deleted[data + 4074..data + 4096],
        &inserted[data + 4074..data + 4096]
    );
    Source {
        before,
        inserted,
        deleted,
    }
}
