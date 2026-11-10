//! Exact current desktop draft defaults, independently authored in raw metadata.
pub(super) mod initial;
use super::{PAGE, Target, append_profile, put16, put32, word};

/// Actual desktop ID/PrimaryKey names and ordinary field properties. This does
/// not assert that Access requires the generated field name to remain fixed.
pub fn append(before: &[u8], name: &str, target: Target) -> Vec<u8> {
    assert!(matches!(
        target,
        Target::GeneratedNumber | Target::GeneratedText
    ));
    append_profile(before, name, target, true)
}

fn name(text: &str) -> Vec<u8> {
    let payload = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut result = (payload.len() as u16).to_le_bytes().to_vec();
    result.extend(payload);
    result
}

fn block(kind: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((payload.len() + 6) as u32).to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend(payload);
    bytes
}

pub(super) fn properties(target: Target) -> Vec<u8> {
    let (field, second) = match target {
        Target::GeneratedNumber => ("Score", "DefaultValue"),
        Target::GeneratedText => ("Name", "AllowZeroLength"),
        _ => panic!("closed desktop generated declaration"),
    };
    let mut dictionary = name("Required");
    dictionary.extend(name(second));
    let field_name = name(field);
    let mut group = ((4 + field_name.len()) as u32).to_le_bytes().to_vec();
    group.extend(field_name);
    group.extend([9, 0, 1, 1, 0, 0, 1, 0, 0]); // Required=False.
    if matches!(target, Target::GeneratedNumber) {
        group.extend([10, 0, 1, 12, 1, 0, 2, 0, b'0', 0]);
    } else {
        group.extend([9, 0, 1, 1, 1, 0, 1, 0, 255]);
    }
    let mut body = b"MR2\0".to_vec();
    body.extend(block(128, &dictionary));
    body.extend(block(1, &group));
    body
}

pub(super) fn definition(page: &mut [u8]) {
    let old_length = u32::from_le_bytes(page[8..12].try_into().unwrap()) as usize;
    let count = word(page, 45);
    let mut index = 75 + count * 25;
    for _ in 0..count {
        index += 2 + word(page, index);
    }
    let label = index + 52 + 28;
    assert_eq!(word(page, label), 18);
    assert_eq!(label + 2 + 18 + 2, old_length);
    let old_name = name("PrimaryId");
    assert_eq!(&page[label..label + old_name.len()], old_name);
    let new_name = name("PrimaryKey");
    let end = label + new_name.len();
    page[label..end].copy_from_slice(&new_name);
    page[end..end + 2].copy_from_slice(&[255, 255]);
    let length = end + 2;
    put32(page, 8, length as u32);
    put16(page, 2, (PAGE - length - 8) as u16);
}
