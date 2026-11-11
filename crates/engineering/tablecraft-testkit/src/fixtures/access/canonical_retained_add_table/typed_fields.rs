//! Literal typed saved-field ADD; no engine declaration or serialization imports.
use crate::fixtures::access::{
    generated_record_insert::placement::{self, Pages},
    put16, put32, raw_catalog_properties, raw_forwarding_chain, raw_record_update,
};

const PAGE: usize = 4096;
const FIELD_BYTES: usize = 25;

/// Ordinary gallery declaration, including actual native variable parameters.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Boolean,
    Byte,
    Int16,
    Int32,
    Int64,
    Single,
    Double,
    Currency,
    DateTime,
    Guid,
    Decimal { precision: u8, scale: u8 },
    ShortText { units: u16, compression: bool },
}

pub const KINDS: [Kind; 12] = [
    Kind::Boolean,
    Kind::Byte,
    Kind::Int16,
    Kind::Int32,
    Kind::Int64,
    Kind::Single,
    Kind::Double,
    Kind::Currency,
    Kind::DateTime,
    Kind::Guid,
    Kind::Decimal {
        precision: 28,
        scale: 4,
    },
    Kind::ShortText {
        units: 255,
        compression: true,
    },
];

impl Kind {
    fn storage(self) -> (u8, Option<usize>) {
        match self {
            Self::Boolean => (1, Some(0)),
            Self::Byte => (2, Some(1)),
            Self::Int16 => (3, Some(2)),
            Self::Int32 => (4, Some(4)),
            Self::Int64 => (19, Some(8)),
            Self::Single => (6, Some(4)),
            Self::Double => (7, Some(8)),
            Self::Currency => (5, Some(8)),
            Self::DateTime => (8, Some(8)),
            Self::Guid => (15, Some(16)),
            Self::Decimal { .. } => (16, None),
            Self::ShortText { .. } => (10, None),
        }
    }

    fn default_expression(self) -> Option<String> {
        match self {
            Self::Boolean => Some("False".into()),
            Self::Byte | Self::Int16 | Self::Int32 | Self::Int64 => Some("0".into()),
            Self::Single | Self::Double => Some("0e0".into()),
            Self::Currency => Some("0.0000".into()),
            Self::Decimal { precision, scale } => {
                assert!((1..=28).contains(&precision) && scale <= precision);
                Some(if scale == 0 {
                    "0".into()
                } else {
                    format!("0.{}", "0".repeat(usize::from(scale)))
                })
            }
            Self::Guid | Self::DateTime | Self::ShortText { .. } => None,
        }
    }
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn wide(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn append_name(bytes: &mut Vec<u8>, name: &str) {
    let payload = wide(name);
    bytes.extend((payload.len() as u16).to_le_bytes());
    bytes.extend(payload);
}

fn properties(original: &[u8], name: &str, kind: Kind) -> Vec<u8> {
    assert_eq!(&original[..4], b"MR2\0");
    let dictionary_end = 4 + u32::from_le_bytes(original[4..8].try_into().unwrap()) as usize;
    assert_eq!(&original[8..10], &[128, 0]);
    let mut names = Vec::new();
    let mut position = 10;
    while position < dictionary_end {
        let length = word(original, position);
        let units = original[position + 2..position + 2 + length]
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(u16::from_le_bytes)
            .collect::<Vec<_>>();
        names.push(String::from_utf16(&units).unwrap());
        position += 2 + length;
    }
    assert_eq!(position, dictionary_end);
    let default = kind.default_expression();
    let mut entries = vec![("Required", 1_u8, vec![0_u8])];
    if let Some(default) = default {
        entries.push(("DefaultValue", 12, wide(&default)));
    } else if matches!(kind, Kind::ShortText { .. }) {
        entries.push(("AllowZeroLength", 1, vec![255]));
    }
    let mut dictionary = original[..dictionary_end].to_vec();
    let mut indexed = Vec::new();
    for (key, native, payload) in entries {
        let index = if let Some(index) = names.iter().position(|old| old.eq_ignore_ascii_case(key))
        {
            index
        } else {
            let index = names.len();
            append_name(&mut dictionary, key);
            names.push(key.into());
            index
        };
        indexed.push((index, native, payload));
    }
    let length = dictionary.len() - 4;
    put32(&mut dictionary, 4, length as u32);
    dictionary.extend_from_slice(&original[dictionary_end..]);
    let mut group = vec![0; 6];
    group[4] = 1;
    let label = wide(name);
    group.extend((label.len() as u32 + 6).to_le_bytes());
    group.extend((label.len() as u16).to_le_bytes());
    group.extend(label);
    indexed.sort_by_key(|entry| entry.0);
    for (index, native, payload) in indexed {
        group.extend((payload.len() as u16 + 8).to_le_bytes());
        group.extend([1, native]);
        group.extend((index as u16).to_le_bytes());
        group.extend((payload.len() as u16).to_le_bytes());
        group.extend(payload);
    }
    let length = group.len();
    put32(&mut group, 0, length as u32);
    dictionary.extend(group);
    dictionary
}

/// Independent complete image with exact old records, active descriptors and snapshots.
pub fn after(
    before: &[u8],
    definition: u32,
    catalog_root: usize,
    name: &str,
    kind: Kind,
) -> Vec<u8> {
    let result = descriptor_after(before, definition, name, kind, [9, 4, 0, 1]);
    let body = properties(
        &raw_catalog_properties::body(before, catalog_root),
        name,
        kind,
    );
    let candidate = raw_catalog_properties::candidate(before, body.len());
    raw_catalog_properties::append(before, result, catalog_root, &body, candidate)
}

// The gallery and explicit declaration fixtures share only literal physical
// framing. Property snapshots remain solely in the gallery operation above.
pub(super) fn descriptor_after(
    before: &[u8],
    definition: u32,
    name: &str,
    kind: Kind,
    text_order: [u8; 4],
) -> Vec<u8> {
    assert!(name.is_ascii() && !name.is_empty() && name.len() <= 64);
    let base = definition as usize * PAGE;
    let original = &before[base..base + PAGE];
    let count = word(original, 45);
    let high = word(original, 41);
    let variables = word(original, 43);
    let indexes = u32::from_le_bytes(original[51..55].try_into().unwrap()) as usize;
    let fields = 63 + indexes * 12;
    let names = fields + count * FIELD_BYTES;
    let mut end_names = names;
    for _ in 0..count {
        end_names += 2 + word(original, end_names);
    }
    let length = u32::from_le_bytes(original[8..12].try_into().unwrap()) as usize;
    let mut fixed = 0;
    let mut variable = 0;
    let mut stored_id = 0;
    for at in (fields..names).step_by(FIELD_BYTES) {
        stored_id = stored_id.max(word(original, at + 9) + 1);
        if original[at + 15] & 1 != 0 {
            if original[at] != 1 {
                fixed = fixed.max(word(original, at + 21) + word(original, at + 23));
            }
        } else {
            variable = variable.max(word(original, at + 7) + 1);
        }
    }
    let (native, width) = kind.storage();
    let mut descriptor = [0_u8; FIELD_BYTES];
    descriptor[0] = native;
    put32(&mut descriptor, 1, 1625);
    put16(&mut descriptor, 5, high as u16);
    put16(&mut descriptor, 7, variable as u16);
    put16(&mut descriptor, 9, stored_id as u16);
    descriptor[15] = if width.is_some() { 3 } else { 2 };
    match kind {
        Kind::Decimal { precision, scale } => {
            descriptor[11] = precision;
            descriptor[12] = scale;
            put16(&mut descriptor, 23, 17);
        }
        Kind::ShortText { units, compression } => {
            assert!((1..=255).contains(&units));
            descriptor[11..15].copy_from_slice(&text_order);
            descriptor[16] = u8::from(compression);
            put16(&mut descriptor, 23, units * 2);
        }
        _ => {
            let width = width.unwrap();
            put16(
                &mut descriptor,
                21,
                if width == 0 { 0 } else { fixed as u16 },
            );
            put16(&mut descriptor, 23, width as u16);
        }
    }
    let mut page = original[..names].to_vec();
    page.extend(descriptor);
    page.extend_from_slice(&original[names..end_names]);
    append_name(&mut page, name);
    page.extend_from_slice(&original[end_names..length]);
    let new_length = page.len();
    assert!(new_length <= PAGE - 8);
    put16(&mut page, 2, (PAGE - 8 - new_length) as u16);
    put32(&mut page, 8, new_length as u32);
    put16(&mut page, 41, (high + 1) as u16);
    put16(&mut page, 45, (count + 1) as u16);
    put16(
        &mut page,
        43,
        (variables + usize::from(width.is_none())) as u16,
    );
    page.extend_from_slice(&original[new_length..]);
    let mut result = before.to_vec();
    result[base..base + PAGE].copy_from_slice(&page);
    result
}

/// Preserve a short two-field row while setting the previously unrepresented Boolean bit.
pub fn boolean_historical(
    before: &[u8],
    definition: u32,
    root: usize,
    ignored_new_bit: bool,
) -> Vec<u8> {
    let base = definition as usize * PAGE;
    assert_eq!(word(before, base + 41), 3);
    assert_eq!(before[base + 75 + 50], 1);
    let range = raw_forwarding_chain::terminal_range(before, definition + 1, root);
    assert_eq!(word(before, range.start), 2);
    assert_eq!(range.len(), 11);
    let mut result = before.to_vec();
    result[range.end - 1] = (result[range.end - 1] & !4) | if ignored_new_bit { 4 } else { 0 };
    result
}

/// The absent Boolean becomes physically represented true; other stored bits survive.
pub fn boolean_materialized_true(before: &[u8], definition: u32, root: usize) -> Vec<u8> {
    let range = raw_forwarding_chain::terminal_range(before, definition + 1, root);
    assert_eq!(word(before, range.start), 2);
    assert_eq!(range.len(), 11);
    let mut result = before.to_vec();
    put16(&mut result, range.start, 3);
    result[range.end - 1] = before[range.end - 1] | 4;
    result
}

/// Literal fresh three-field row under the newly saved gallery declaration.
/// Old rows and their ignored payloads remain untouched; only new rows use defaults.
pub fn append_omitted(before: &[u8], definition: u32, kind: Kind) -> Vec<u8> {
    let base = definition as usize * PAGE;
    assert_eq!(word(before, base + 41), 3);
    assert_eq!(word(before, base + 45), 3);
    let field = base + 75 + 50;
    let (native, width) = kind.storage();
    assert_eq!(before[field], native);
    assert_eq!(word(before, field + 5), 2);
    let pages = Pages {
        definition: definition as usize,
        data: definition as usize + 1,
        index: definition as usize + 2,
    };
    let id = placement::next_id(before, pages);
    let mut row = vec![3, 0];
    row.extend(id.to_le_bytes());
    row.extend(0_i32.to_le_bytes());
    let mut mask = 3_u8;
    if let Some(width) = width {
        row.resize(10 + width, 0);
        if kind.default_expression().is_some() && !matches!(kind, Kind::Boolean) {
            mask |= 4;
        }
    } else {
        assert_eq!(word(before, base + 43), 1);
        if matches!(kind, Kind::Decimal { .. }) {
            row.extend([0; 17]);
            mask |= 4;
        }
        let end = row.len();
        row.extend((end as u16).to_le_bytes());
        row.extend(10_u16.to_le_bytes());
        row.extend(1_u16.to_le_bytes());
    }
    row.push(mask);
    placement::append(before, pages, id, &row)
}

/// Change only a dictionary key's ASCII case in the actual current property carrier.
pub fn lowercase_required(before: &[u8], catalog_root: usize) -> Vec<u8> {
    let range = raw_forwarding_chain::terminal_range(before, 4, catalog_root);
    let row = &before[range.clone()];
    let footer = row.len() - 5;
    let descriptor = word(row, footer - 18);
    let tag = u32::from_le_bytes(row[descriptor..descriptor + 4].try_into().unwrap());
    let body_start = match tag & 0xc000_0000 {
        0x8000_0000 => range.start + descriptor + 12,
        0x4000_0000 => {
            let reference =
                u32::from_le_bytes(row[descriptor + 4..descriptor + 8].try_into().unwrap());
            let page = (reference >> 8) as usize * PAGE;
            page + word(before, page + 14 + 2 * (reference as usize & 255))
        }
        _ => panic!("expected authored Inline or Single property body"),
    };
    let dictionary_end = body_start
        + 4
        + u32::from_le_bytes(before[body_start + 4..body_start + 8].try_into().unwrap()) as usize;
    let required = wide("Required");
    let mut position = body_start + 10;
    let mut result = before.to_vec();
    let mut found = false;
    while position < dictionary_end {
        let length = word(before, position);
        if before[position + 2..position + 2 + length] == required {
            assert!(!found);
            found = true;
            result[position + 2] = b'r';
        }
        position += 2 + length;
    }
    assert!(found);
    result
}

/// Independently materialize or replace the actual added Currency tail.
/// NULL preserves ignored present payload; absent NULL is an exact no-op.
pub fn update_currency(before: &[u8], definition: u32, root: usize, value: Option<i64>) -> Vec<u8> {
    let base = definition as usize * PAGE;
    assert_eq!(word(before, base + 41), 3);
    assert_eq!(word(before, base + 45), 3);
    assert_eq!(word(before, base + 43), 0);
    let field = base + 75 + 50;
    assert_eq!(before[field], 5);
    assert_eq!(word(before, field + 5), 2);
    assert_eq!(word(before, field + 21), 8);
    assert_eq!(word(before, field + 23), 8);
    raw_record_update::historical_currency(before, definition + 1, root, field, value)
}

/// Independent Boolean edit after ADD: absence is false, true materializes the bit.
pub fn update_boolean(before: &[u8], definition: u32, root: usize, value: bool) -> Vec<u8> {
    let base = definition as usize * PAGE;
    assert_eq!(word(before, base + 41), 3);
    assert_eq!(before[base + 75 + 50], 1);
    let range = raw_forwarding_chain::terminal_range(before, definition + 1, root);
    match word(before, range.start) {
        2 if value => boolean_materialized_true(before, definition, root),
        2 => before.to_vec(),
        3 => {
            assert_eq!(range.len(), 11);
            let mut result = before.to_vec();
            result[range.end - 1] = (before[range.end - 1] & !4) | if value { 4 } else { 0 };
            result
        }
        _ => panic!("expected the known short or materialized gallery row"),
    }
}

fn boolean_definition(before: &[u8], definition: u32) {
    let page = definition as usize * PAGE;
    let active = word(before, page + 45);
    assert!(active > 0);
    let indexes = u32::from_le_bytes(before[page + 51..page + 55].try_into().unwrap()) as usize;
    let tail = page + 63 + 12 * indexes + (active - 1) * 25;
    assert_eq!(before[tail], 1);
    assert_eq!(&before[tail + 15..tail + 17], &[3, 0]);
    assert_eq!(word(before, tail + 23), 0);
}

/// Observe the stored logical row under the actual newly added Boolean declaration.
pub fn boolean_record(before: &[u8], definition: u32, root: usize) -> &[u8] {
    boolean_definition(before, definition);
    raw_forwarding_chain::record(before, definition + 1, root)
}

/// Replace the existing terminal at the same footprint without changing its chain.
pub fn replace_boolean_record(before: &[u8], definition: u32, root: usize, row: &[u8]) -> Vec<u8> {
    boolean_definition(before, definition);
    raw_forwarding_chain::replace_terminal(before, definition + 1, root, row)
}

/// Place a reconstructed Boolean row with the independent retained forwarding law.
pub fn place_boolean_record(before: &[u8], definition: u32, root: usize, row: &[u8]) -> Vec<u8> {
    boolean_definition(before, definition);
    raw_forwarding_chain::place_terminal(before, definition + 1, root, row)
}
