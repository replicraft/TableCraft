//! Independent generated-primary ADD column bytes; no production codecs.
use super::{canonical_creation_defaults, canonical_initial_table_creation, put16, put32};

const PAGE: usize = 4096;

/// Canonical generated primary plus two LONG fields, with an inline default zero.
/// The inline MR2 bag comes from the existing independent native default fixture;
/// changing its literal 7 to 0 retains its exact native framing and storage role.
/// Jackcess's public ADD operation independently qualifies the complete AFTER.
pub fn default_zero_source() -> Vec<u8> {
    let mut bytes = canonical_initial_table_creation::source(
        canonical_initial_table_creation::Case::LongInteger,
    )
    .bytes;
    let donor = canonical_creation_defaults::source(canonical_creation_defaults::Case::EmptySeven);
    let bag = donor.property_bytes.as_ref().unwrap();
    assert_eq!(&bag[bag.len() - 2..], &[b'7', 0]);
    let mut catalog = donor.bytes[4 * PAGE..5 * PAGE].to_vec();
    let position = catalog
        .windows(bag.len())
        .position(|actual| actual == bag)
        .unwrap();
    catalog[position + bag.len() - 2] = b'0';
    bytes[4 * PAGE..5 * PAGE].copy_from_slice(&catalog);
    bytes
}

/// Append an ordinary nullable LONG field with no declared default. Literal
/// metadata and tail relocation are independent of engine decoders/serializers.
pub fn append(before: &[u8], name: &str) -> Vec<u8> {
    assert_eq!(before.len(), 10 * PAGE);
    let original = &before[7 * PAGE..8 * PAGE];
    let count = u16::from_le_bytes(original[41..43].try_into().unwrap()) as usize;
    let variables = u16::from_le_bytes(original[43..45].try_into().unwrap());
    assert!((1..16).contains(&count));
    assert_eq!(&original[16..24], &[0; 8]);
    assert_eq!(&original[47..55], &[1, 0, 0, 0, 1, 0, 0, 0]);
    let length = u32::from_le_bytes(original[8..12].try_into().unwrap()) as usize;
    let names = 75 + count * 25;
    let mut index_tail = names;
    for _ in 0..count {
        let bytes =
            u16::from_le_bytes(original[index_tail..index_tail + 2].try_into().unwrap()) as usize;
        index_tail += 2 + bytes;
    }
    let mut fixed_end = 0;
    for at in (75..names).step_by(25) {
        if original[at + 15] & 1 != 0 && original[at] != 1 {
            let offset =
                u16::from_le_bytes(original[at + 21..at + 23].try_into().unwrap()) as usize;
            let width = u16::from_le_bytes(original[at + 23..at + 25].try_into().unwrap()) as usize;
            fixed_end = fixed_end.max(offset + width);
        }
    }
    let mut definition = original[..names].to_vec();
    let mut field = [0_u8; 25];
    field[0] = 4;
    put32(&mut field, 1, 1625);
    put16(&mut field, 5, count as u16);
    put16(&mut field, 7, variables);
    put16(&mut field, 9, count as u16);
    field[15] = 3;
    put16(&mut field, 21, fixed_end as u16);
    put16(&mut field, 23, 4);
    definition.extend(field);
    definition.extend(&original[names..index_tail]);
    let units = name.encode_utf16().collect::<Vec<_>>();
    definition.extend((units.len() as u16 * 2).to_le_bytes());
    for unit in units {
        definition.extend(unit.to_le_bytes());
    }
    definition.extend(&original[index_tail..length]);
    let length = definition.len();
    put16(&mut definition, 2, (PAGE - length - 8) as u16);
    put32(&mut definition, 8, length as u32);
    put16(&mut definition, 41, (count + 1) as u16);
    put16(&mut definition, 45, (count + 1) as u16);
    definition.resize(PAGE, 0);
    let mut after = before.to_vec();
    after[7 * PAGE..8 * PAGE].copy_from_slice(&definition);
    after
}
