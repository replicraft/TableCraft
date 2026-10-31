//! Additive catalog/payload originals built with the actual literal owners.
use super::super::super::{guard_sources::model as original, inspect::payload_fixture};
use super::super::model as observed;
use tablecraft_testkit::access as raw;

const PAGE_BYTES: usize = 4096;
const HEADER_VERSION_OFFSET: usize = 20;
const CATALOG_PAGE: usize = 4;
const CATALOG_DEFINITION_PAGE: usize = 2;
const CATALOG_RECORD_COUNT: usize = 16;
const ROOT_SLOT_WORD: usize = 18;
const PREVIOUS_SLOT_WORD: usize = 16;
const FIXED_PREFIX: usize = 16;
const ID_OFFSET: usize = 2;
const PARENT_OFFSET: usize = 6;
const TYPE_OFFSET: usize = 10;
const FLAGS_OFFSET: usize = 12;
const NAME_OFFSET: usize = 16;
const NAME_BYTES: usize = 7;
const NAME_PREFIX_BYTES: usize = 2;
const SLOT_MASK: usize = 0x0fff;

#[derive(Clone, Copy, Debug)]
pub(super) enum Case {
    MissingColumns,
    NullExtra,
    EmptyExtra,
    ChildrenAndDangling,
    DuplicateRootNames,
}

pub(super) const CASES: [Case; 5] = [
    Case::MissingColumns,
    Case::NullExtra,
    Case::EmptyExtra,
    Case::ChildrenAndDangling,
    Case::DuplicateRootNames,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Presence {
    Missing,
    Absent,
    Present,
}

pub(super) struct Child {
    pub id: i32,
    pub kind: i16,
    pub flags: u32,
    pub name: &'static str,
    pub presence: Presence,
    pub body: Option<Vec<u8>>,
    pub module: Option<Vec<u8>>,
    pub extra: Option<Vec<u8>>,
}

pub(super) struct Source {
    pub bytes: Vec<u8>,
    pub id: u64,
    pub body: Option<Vec<u8>>,
    pub module: Option<Vec<u8>>,
    pub extra: Option<Vec<u8>>,
    pub missing_columns: bool,
    pub children: Vec<Child>,
}

fn selected(bytes: &[u8]) -> Vec<u8> {
    let page = CATALOG_PAGE * PAGE_BYTES;
    let start = usize::from(u16::from_le_bytes(
        bytes[page + ROOT_SLOT_WORD..page + ROOT_SLOT_WORD + 2]
            .try_into()
            .unwrap(),
    )) & SLOT_MASK;
    let end = usize::from(u16::from_le_bytes(
        bytes[page + PREVIOUS_SLOT_WORD..page + PREVIOUS_SLOT_WORD + 2]
            .try_into()
            .unwrap(),
    )) & SLOT_MASK;
    bytes[page + start..page + end].to_vec()
}

fn record(code: u8, child: &Child, parent: i32) -> Vec<u8> {
    let fixture = payload_fixture::database(
        code,
        &[
            child.body.as_deref().map(payload_fixture::inline),
            child.module.as_deref().map(payload_fixture::inline),
            child.extra.as_deref().map(payload_fixture::inline),
        ],
    );
    let (local, _, _) = original::source(code, "BeforeChange", "<Statements/>");
    let local = selected(&local);
    let mut row = selected(&fixture);
    row[..FIXED_PREFIX].copy_from_slice(&local[..FIXED_PREFIX]);
    raw::put32(&mut row, ID_OFFSET, child.id as u32);
    raw::put32(&mut row, PARENT_OFFSET, parent as u32);
    raw::put16(&mut row, TYPE_OFFSET, child.kind as u16);
    raw::put32(&mut row, FLAGS_OFFSET, child.flags);
    assert_eq!(
        &row[NAME_OFFSET..NAME_OFFSET + NAME_BYTES],
        b"\xff\xfeItems"
    );
    assert_eq!(child.name.len(), 5);
    row[NAME_OFFSET + NAME_PREFIX_BYTES..NAME_OFFSET + NAME_BYTES]
        .copy_from_slice(child.name.as_bytes());
    row
}

pub(super) fn source(code: u8, case: Case) -> Source {
    if matches!(case, Case::MissingColumns) {
        let mut bytes = raw::image(false, false);
        bytes[HEADER_VERSION_OFFSET] = code;
        return Source {
            bytes,
            id: 100,
            body: None,
            module: None,
            extra: None,
            missing_columns: true,
            children: vec![Child {
                id: -1,
                kind: -32768,
                flags: 0,
                name: "Form",
                presence: Presence::Missing,
                body: None,
                module: None,
                extra: None,
            }],
        };
    }

    let extra = match case {
        Case::NullExtra => None,
        Case::EmptyExtra => Some(Vec::new()),
        _ => Some(observed::source(code, observed::Case::NonstandardEventProperty).bag),
    };
    let body = Some(b"opaque root body\x00".to_vec());
    let module = Some(b"opaque root module\xff".to_vec());
    let root = Child {
        id: 10,
        kind: 1,
        flags: 0,
        name: "Items",
        presence: Presence::Present,
        body: body.clone(),
        module: module.clone(),
        extra: extra.clone(),
    };
    let (mut bytes, _, _) = original::source(code, "BeforeChange", "<Statements/>");
    let mut records = vec![
        (0, raw::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects")),
        (
            0,
            raw::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        ),
        (0, record(code, &root, 0)),
    ];
    let mut children = Vec::new();
    if matches!(case, Case::ChildrenAndDangling) {
        children = vec![
            Child {
                id: 11,
                kind: -32766,
                flags: 0,
                name: "Child",
                presence: Presence::Present,
                body: Some(b"opaque child body".to_vec()),
                module: Some(b"opaque child module".to_vec()),
                extra: Some(b"opaque child Extra; never root MR2".to_vec()),
            },
            Child {
                id: 12,
                kind: 12345,
                flags: 0x0040,
                name: "Child",
                presence: Presence::Absent,
                body: None,
                module: None,
                extra: None,
            },
        ];
        for child in &children {
            records.push((0, record(code, child, 10)));
        }
        let unrelated = Child {
            id: 13,
            kind: -32766,
            flags: 0,
            name: "Other",
            presence: Presence::Present,
            body: Some(b"unrelated body".to_vec()),
            module: None,
            extra: Some(b"unrelated opaque Extra".to_vec()),
        };
        records.push((0, record(code, &unrelated, 999)));
    }
    if matches!(case, Case::DuplicateRootNames) {
        let second = Child {
            id: 20,
            kind: 1,
            flags: 0,
            name: "Items",
            presence: Presence::Present,
            body: Some(b"other root body".to_vec()),
            module: Some(b"other root module".to_vec()),
            extra: Some(b"other opaque Extra".to_vec()),
        };
        records.push((0, record(code, &second, 0)));
    }
    let page = CATALOG_PAGE * PAGE_BYTES;
    bytes[page..page + PAGE_BYTES].copy_from_slice(&raw::data_page(2, &records));
    raw::put32(
        &mut bytes,
        CATALOG_DEFINITION_PAGE * PAGE_BYTES + CATALOG_RECORD_COUNT,
        records.len() as u32,
    );
    Source {
        bytes,
        id: 10,
        body,
        module,
        extra,
        missing_columns: false,
        children,
    }
}
