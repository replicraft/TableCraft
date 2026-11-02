//! Independent inline-map SOURCE derivatives, never normal write authority.
//!
//! Only the already qualified canonical BEFORE envelope is reused. Map records,
//! physical roles and the relocated DATA page are declared here without calling
//! an allocation reader, creator, planner or ordinary-content validator.
use super::{canonical_creation, data_page, put16, put32};

const PAGE: usize = 4096;
const MAP_PAGE: usize = 3;
const USER_DEFINITION: u32 = 7;
const USER_DATA: u32 = 95;
const PAGE_COUNT: usize = 96;

/// Finite source identities for explicit read qualification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// One row on page95, with a 96-bit GLOBAL window.
    WideOne,
    /// Endpoint rows on page95, independently preserving their raw record bytes.
    WideExtrema,
    /// Owned base64 and candidate base88 represent the same page95.
    DifferentWindows,
    /// A 128-bit GLOBAL window marks pages96..127 free beyond current EOF.
    FutureFree,
    /// Empty LVAL windows may have no bitmap or all-zero out-of-file windows.
    EmptyWindows,
    /// An owned SET bit names page96 beyond EOF.
    OwnedPastEof,
    /// A candidate SET bit names page96 beyond EOF.
    CandidatePastEof,
    /// GLOBAL does not represent physical pages64..95.
    MissingGlobalCoverage,
    /// GLOBAL declares the live catalog definition page free.
    FreeMetadata,
    /// Owned and candidate references select the same physical map cell.
    MapAlias,
    /// An otherwise empty map uses an indirect tag outside the proposed grammar.
    Indirect,
    /// GLOBAL base8 cannot represent the allocated header page0.
    ShiftedGlobal,
}

/// Authoring order; five positive hypotheses and seven refusal controls.
pub const CASES: [Case; 12] = [
    Case::WideOne,
    Case::WideExtrema,
    Case::DifferentWindows,
    Case::FutureFree,
    Case::EmptyWindows,
    Case::OwnedPastEof,
    Case::CandidatePastEof,
    Case::MissingGlobalCoverage,
    Case::FreeMetadata,
    Case::MapAlias,
    Case::Indirect,
    Case::ShiftedGlobal,
];

impl Case {
    /// Stable identity for later explicit export.
    pub const fn id(self) -> &'static str {
        match self {
            Self::WideOne => "wide-one",
            Self::WideExtrema => "wide-extrema",
            Self::DifferentWindows => "different-windows",
            Self::FutureFree => "future-free",
            Self::EmptyWindows => "empty-windows",
            Self::OwnedPastEof => "owned-past-eof",
            Self::CandidatePastEof => "candidate-past-eof",
            Self::MissingGlobalCoverage => "missing-global-coverage",
            Self::FreeMetadata => "free-metadata",
            Self::MapAlias => "map-alias",
            Self::Indirect => "indirect",
            Self::ShiftedGlobal => "shifted-global",
        }
    }
}

/// Exact source image, semantic rows and independently declared physical facts.
pub struct Source {
    /// Complete page-aligned image, including all zero free holes.
    pub bytes: Vec<u8>,
    /// Original user table name.
    pub table: &'static str,
    /// Original ordered column names.
    pub columns: Vec<String>,
    /// Original row values in page95 slot order.
    pub rows: Vec<Vec<i32>>,
    /// Original definition page, unchanged by relocation.
    pub definition: u32,
    /// Only user DATA page.
    pub data: u32,
    /// Declared allocated physical pages, regardless of an adversarial map bit.
    pub allocated: Vec<u32>,
    /// Declared zero free physical holes.
    pub free: Vec<u32>,
    /// Exact GLOBAL row bytes, separately from ordinary map records.
    pub global: Vec<u8>,
    /// Exact map-container rows0..15, preserving the catalog LVAL identities.
    pub maps: Vec<Vec<u8>>,
    /// Expected proposed grammar disposition, not an admission capability.
    pub ordinary: bool,
}

fn inline(base: u32, bitmap_bytes: usize, members: &[u32]) -> Vec<u8> {
    let mut row = vec![0; 5 + bitmap_bytes];
    put32(&mut row, 1, base);
    for &member in members {
        let relative = usize::try_from(member.checked_sub(base).unwrap()).unwrap();
        assert!(relative < bitmap_bytes * 8);
        row[5 + relative / 8] |= 1 << (relative % 8);
    }
    row
}

fn packed(owner: u32, rows: &[Vec<u8>]) -> Vec<u8> {
    let records = rows.iter().cloned().map(|row| (0, row)).collect::<Vec<_>>();
    let used = 14 + 2 * rows.len() + rows.iter().map(Vec::len).sum::<usize>();
    assert!(used <= PAGE);
    let mut page = data_page(owner, &records);
    put16(&mut page, 2, (PAGE - used) as u16);
    page
}

/// Author a complete source without consulting any proposed production checker.
pub fn source(case: Case) -> Source {
    let seed = if case == Case::WideExtrema {
        canonical_creation::Case::Extrema
    } else {
        canonical_creation::Case::One
    };
    let original = canonical_creation::source(seed);
    let mut bytes = original.bytes;
    let data = bytes[8 * PAGE..9 * PAGE].to_vec();
    bytes.resize(PAGE_COUNT * PAGE, 0);
    bytes[8 * PAGE..9 * PAGE].fill(0);
    bytes[USER_DATA as usize * PAGE..(USER_DATA as usize + 1) * PAGE].copy_from_slice(&data);

    // Header/GLOBAL/catalog/maps/three system definitions/user definition stay
    // pages0..7. Only the DATA allocation moves; TD7 references retain slots3/7.
    let allocated = (0..8).chain([USER_DATA]).collect::<Vec<u32>>();
    let free = (8..USER_DATA).collect::<Vec<u32>>();
    let mut global = inline(0, 12, &free);
    let mut maps = vec![
        inline(0, 8, &[4]),
        inline(0, 8, &[]),
        inline(0, 8, &[]),
        inline(64, 4, &[USER_DATA]),
        inline(0, 8, &[4]),
        inline(0, 8, &[]),
        inline(0, 8, &[]),
        inline(64, 4, &[USER_DATA]),
    ];
    maps.extend((0..8).map(|_| inline(0, 8, &[])));

    match case {
        Case::WideOne | Case::WideExtrema => {}
        Case::DifferentWindows => {
            maps[7] = inline(88, 1, &[USER_DATA]);
        }
        Case::FutureFree => {
            let future_free = (8..USER_DATA).chain(96..128).collect::<Vec<u32>>();
            global = inline(0, 16, &future_free);
        }
        Case::EmptyWindows => {
            maps[8] = inline(4096, 0, &[]);
            maps[9] = inline(8192, 3, &[]);
        }
        Case::OwnedPastEof => {
            maps[3] = inline(64, 5, &[USER_DATA, 96]);
        }
        Case::CandidatePastEof => {
            maps[7] = inline(64, 5, &[USER_DATA, 96]);
        }
        Case::MissingGlobalCoverage => {
            global = inline(0, 8, &(8..64).collect::<Vec<_>>());
        }
        Case::FreeMetadata => {
            global[5] |= 1 << 2;
        }
        Case::MapAlias => {
            put32(
                &mut bytes,
                USER_DEFINITION as usize * PAGE + 59,
                (3 << 8) | 3,
            );
        }
        Case::Indirect => {
            maps[8] = vec![1, 0, 0, 0, 0];
        }
        Case::ShiftedGlobal => {
            global = inline(8, 11, &free);
        }
    }

    bytes[PAGE..2 * PAGE].copy_from_slice(&packed(1, &[global.clone()]));
    bytes[MAP_PAGE * PAGE..(MAP_PAGE + 1) * PAGE].copy_from_slice(&packed(0, &maps));

    Source {
        bytes,
        table: original.table,
        columns: original.columns,
        rows: original.rows,
        definition: USER_DEFINITION,
        data: USER_DATA,
        allocated,
        free,
        global,
        maps,
        ordinary: matches!(
            case,
            Case::WideOne
                | Case::WideExtrema
                | Case::DifferentWindows
                | Case::FutureFree
                | Case::EmptyWindows
        ),
    }
}
