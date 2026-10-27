//! Read/DELETE goldens wrapped with explicit writable/free-hint metadata.
use super::{encoded_model, support};
pub use encoded_model::{Model, budget, capture, entries, wire};

pub fn qualify(mut model: Model) -> Model {
    model.bytes[10 * 4096 + 115] = 2;
    support::put32(&mut model.bytes, 10 * 4096 + 59, (11 << 8) | 1);
    let start = usize::from(
        u16::from_le_bytes(
            model.bytes[11 * 4096 + 14..11 * 4096 + 16]
                .try_into()
                .unwrap(),
        ) & 0x1fff,
    );
    let owned = model.bytes[11 * 4096 + start..12 * 4096].to_vec();
    let hint = vec![0, 0, 0, 0, 0, 0, 0x10];
    model.bytes[11 * 4096..12 * 4096]
        .copy_from_slice(&support::data_page(0, &[(0, owned), (0, hint)]));
    fix_free(&mut model.bytes, 12);
    model
}

pub fn fix_free(bytes: &mut [u8], number: usize) {
    let page = &mut bytes[number * 4096..(number + 1) * 4096];
    let count = usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap()));
    let first = if count == 0 {
        4096
    } else {
        usize::from(
            u16::from_le_bytes(page[12 + 2 * count..14 + 2 * count].try_into().unwrap()) & 0x1fff,
        )
    };
    support::put16(page, 2, (first - 14 - 2 * count) as u16);
}

pub fn text(family: u8, descending: bool, alias: bool) -> Model {
    qualify(encoded_model::text(family, descending, alias))
}

pub fn binary(descending: bool, alias: bool) -> Model {
    qualify(encoded_model::binary(descending, alias))
}

pub fn tree(model: Model, bounded: bool) -> Model {
    encoded_model::tree(model, bounded)
}

pub fn composite() -> Model {
    qualify(encoded_model::composite())
}
