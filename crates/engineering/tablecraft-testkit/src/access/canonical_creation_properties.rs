//! Independent creation-property SOURCE bytes; no creator/property writer imports.
use super::{
    canonical_native17_creation as native17, canonical_variable_creation as variable, data_page,
    put16,
};
use tablecraft_core::values::Value;

#[derive(Clone, Copy)]
pub enum Case {
    Absent,
    AllowEmptyTrue,
    AllowEmptyFalse,
    RequiredText,
    RequiredEmptyAllowed,
    RequiredFlag,
    RequiredId,
    RequiredNullRefusal,
    EmptyTextRefusal,
}

pub const CASES: [Case; 9] = [
    Case::Absent,
    Case::AllowEmptyTrue,
    Case::AllowEmptyFalse,
    Case::RequiredText,
    Case::RequiredEmptyAllowed,
    Case::RequiredFlag,
    Case::RequiredId,
    Case::RequiredNullRefusal,
    Case::EmptyTextRefusal,
];

impl Case {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::AllowEmptyTrue => "allow-empty-true",
            Self::AllowEmptyFalse => "allow-empty-false",
            Self::RequiredText => "required-text",
            Self::RequiredEmptyAllowed => "required-empty-allowed",
            Self::RequiredFlag => "required-flag",
            Self::RequiredId => "required-id",
            Self::RequiredNullRefusal => "required-null-refusal",
            Self::EmptyTextRefusal => "empty-text-refusal",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Policy {
    pub column: &'static str,
    pub required: Option<bool>,
    pub allow_zero_length: Option<bool>,
}

#[derive(Clone, Copy)]
pub enum Violation {
    Required,
    EmptyText,
}

pub struct Source {
    pub bytes: Vec<u8>,
    pub columns: &'static [variable::Column],
    pub rows: Vec<Vec<Value>>,
    pub policies: Vec<Policy>,
    pub property_bytes: Option<Vec<u8>>,
    pub violation: Option<Violation>,
}

fn word(raw: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(raw[at..at + 2].try_into().unwrap()))
}

fn name(text: &str) -> Vec<u8> {
    let utf16 = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut raw = u16::try_from(utf16.len()).unwrap().to_le_bytes().to_vec();
    raw.extend_from_slice(&utf16);
    raw
}

fn block(kind: u16, body: &[u8]) -> Vec<u8> {
    let mut raw = u32::try_from(body.len() + 6)
        .unwrap()
        .to_le_bytes()
        .to_vec();
    raw.extend_from_slice(&kind.to_le_bytes());
    raw.extend_from_slice(body);
    raw
}

fn properties(policies: &[Policy]) -> Option<Vec<u8>> {
    if policies.is_empty() {
        return None;
    }
    let mut keys = Vec::new();
    if policies.iter().any(|policy| policy.required.is_some()) {
        keys.push("Required");
    }
    if policies
        .iter()
        .any(|policy| policy.allow_zero_length.is_some())
    {
        keys.push("AllowZeroLength");
    }
    assert!(!keys.is_empty());
    let dictionary = keys.iter().flat_map(|key| name(key)).collect::<Vec<_>>();
    let mut raw = b"MR2\0".to_vec();
    raw.extend(block(128, &dictionary));
    for policy in policies {
        let label = name(policy.column);
        let mut body = u32::try_from(label.len() + 4)
            .unwrap()
            .to_le_bytes()
            .to_vec();
        body.extend(label);
        let mut count = 0;
        for (key, value) in [
            ("Required", policy.required),
            ("AllowZeroLength", policy.allow_zero_length),
        ] {
            let Some(value) = value else {
                continue;
            };
            let index = keys.iter().position(|candidate| *candidate == key).unwrap();
            body.extend_from_slice(&9_u16.to_le_bytes());
            body.extend_from_slice(&[1, 1]); // DDL flag1; native property Boolean1.
            body.extend_from_slice(&u16::try_from(index).unwrap().to_le_bytes());
            body.extend_from_slice(&1_u16.to_le_bytes());
            body.push(if value { 255 } else { 0 });
            count += 1;
        }
        assert!(count > 0);
        raw.extend(block(1, &body));
    }
    Some(raw)
}

fn records(page: &[u8]) -> Vec<Vec<u8>> {
    assert_eq!(&page[..2], &[1, 1]);
    assert_eq!(&page[4..8], &2_u32.to_le_bytes());
    assert_eq!(word(page, 12), 8);
    let mut records = Vec::new();
    let mut end = 4096;
    for slot in 0..8 {
        let start = word(page, 14 + 2 * slot);
        assert!(start < end && start >= 30);
        records.push(page[start..end].to_vec());
        end = start;
    }
    assert_eq!(word(page, 2), end - 30);
    records
}

fn attach(original: &[u8], properties: &[u8]) -> Vec<u8> {
    assert_eq!(word(original, 0), 17);
    assert_eq!(&original[original.len() - 3..], &[0xbf, 0, 0]);
    assert_eq!(word(original, original.len() - 5), 11);
    let footer = original.len() - 29;
    let eod = word(original, footer);
    assert_eq!(eod, footer);
    assert_eq!(word(original, footer + 22), 32);
    for slot in 1..11 {
        assert_eq!(word(original, footer + 2 + 2 * (10 - slot)), eod);
    }
    let mut raw = original[..eod].to_vec();
    let property_start = raw.len();
    raw.extend_from_slice(&(0x80000000_u32 | properties.len() as u32).to_le_bytes());
    raw.extend_from_slice(&[0; 8]);
    raw.extend_from_slice(properties);
    let end = raw.len();
    raw.extend_from_slice(&u16::try_from(end).unwrap().to_le_bytes());
    for slot in (0..11).rev() {
        let start = if slot == 0 {
            32
        } else if slot <= 8 {
            property_start
        } else {
            end
        };
        raw.extend_from_slice(&u16::try_from(start).unwrap().to_le_bytes());
    }
    raw.extend_from_slice(&11_u16.to_le_bytes());
    raw.extend_from_slice(&[0xbf, 0x40, 0]);
    assert_eq!(raw.len(), original.len() + properties.len() + 12);
    raw
}

pub fn source(case: Case) -> Source {
    let original_case = match case {
        Case::AllowEmptyFalse | Case::RequiredText | Case::RequiredFlag => variable::Case::Unicode,
        Case::RequiredEmptyAllowed => variable::Case::Empty,
        _ => variable::Case::NullAndEmpty,
    };
    let original = variable::source(original_case);
    let mut bytes = native17::source(original.bytes);
    let (column, required, allow_zero_length) = match case {
        Case::Absent => ("", None, None),
        Case::AllowEmptyTrue => ("TextValue", Some(false), Some(true)),
        Case::AllowEmptyFalse => ("TextValue", Some(false), Some(false)),
        Case::RequiredText => ("TextValue", Some(true), Some(false)),
        Case::RequiredEmptyAllowed | Case::RequiredNullRefusal => {
            ("TextValue", Some(true), Some(true))
        }
        Case::RequiredFlag => ("Flag", Some(true), None),
        Case::RequiredId => ("Id", Some(true), None),
        Case::EmptyTextRefusal => ("TextValue", Some(false), Some(false)),
    };
    let policies = if matches!(case, Case::Absent) {
        vec![]
    } else {
        vec![Policy {
            column,
            required,
            allow_zero_length,
        }]
    };
    let property_bytes = properties(&policies);
    if let Some(property_bytes) = &property_bytes {
        let before = bytes[4 * 4096..5 * 4096].to_vec();
        let mut catalog = records(&before);
        assert_eq!(&catalog[7][2..6], &7_u32.to_le_bytes());
        catalog[7] = attach(&catalog[7], property_bytes);
        let used = 30 + catalog.iter().map(Vec::len).sum::<usize>();
        assert!(used < 4096);
        let rows = catalog.into_iter().map(|row| (0, row)).collect::<Vec<_>>();
        let mut page = data_page(2, &rows);
        put16(&mut page, 2, (4096 - used) as u16);
        bytes[4 * 4096..5 * 4096].copy_from_slice(&page);
    }
    let violation = match case {
        Case::RequiredNullRefusal => Some(Violation::Required),
        Case::EmptyTextRefusal => Some(Violation::EmptyText),
        _ => None,
    };
    Source {
        bytes,
        columns: original.columns,
        rows: original.rows,
        policies,
        property_bytes,
        violation,
    }
}
