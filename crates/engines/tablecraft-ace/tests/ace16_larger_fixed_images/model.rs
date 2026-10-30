//! Independent full files and expected fixed changes, built sequentially.
#[path = "model/placement.rs"]
mod placement;
#[path = "model/scalar.rs"]
mod scalar;

use super::{base, preserved_model, raw};
use std::ops::Range;

pub const PAGE: usize = 4096;
pub const DEFAULT_BYTES: usize = 64 * 1024 * 1024;
pub const EXPLICIT_BYTES: usize = 80 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shape {
    ScalarDefault,
    ScalarFirstHighData,
    ScalarLargeLow,
    ScalarLargeHigh,
    PreservedDefault,
    PreservedHigh,
}

impl Shape {
    pub const ALL: [Self; 6] = [
        Self::ScalarDefault,
        Self::ScalarFirstHighData,
        Self::ScalarLargeLow,
        Self::ScalarLargeHigh,
        Self::PreservedDefault,
        Self::PreservedHigh,
    ];

    pub fn preserved(self) -> bool {
        matches!(self, Self::PreservedDefault | Self::PreservedHigh)
    }

    pub fn length(self) -> usize {
        match self {
            Self::ScalarDefault | Self::PreservedDefault => DEFAULT_BYTES,
            Self::ScalarFirstHighData => DEFAULT_BYTES + PAGE,
            Self::ScalarLargeLow | Self::ScalarLargeHigh => EXPLICIT_BYTES,
            Self::PreservedHigh => DEFAULT_BYTES + 4 * PAGE,
        }
    }

    pub fn data(self) -> usize {
        match self {
            Self::ScalarFirstHighData | Self::ScalarLargeHigh | Self::PreservedHigh => {
                DEFAULT_BYTES / PAGE
            }
            _ => 12,
        }
    }

    pub fn roots(self) -> Vec<usize> {
        match self {
            Self::ScalarLargeHigh => vec![16385, 16386],
            Self::PreservedHigh => vec![16385, 16386, 16387],
            Self::PreservedDefault => vec![27, 28, 29],
            _ => vec![13, 14],
        }
    }
}

pub struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub shape: Shape,
    pub data: usize,
    pub roots: Vec<usize>,
    pub physical_start: usize,
    pub index_map: usize,
    pub global_bitmap: usize,
}

pub fn case(code: u8, shape: Shape) -> Case {
    assert!([5, 6].contains(&code));
    let (mut before, expected, physical_start, index_map) = if shape.preserved() {
        let source = preserved_model::source();
        let names_end = 63
            + 16 * 25
            + [
                "ID",
                "Short_Text",
                "Long_Text",
                "Number",
                "Large_Number",
                "Date_Field",
                "Time_Field",
                "Date_Extended_Field",
                "Time_Extended_Field",
                "Currency_Field",
                "Yes_Field",
                "No_Field",
                "OLE_Object_Field",
                "Hyperlink_Field",
                "Attachment_Field",
                "Calculated",
            ]
            .iter()
            .map(|name| 2 + name.len() * 2)
            .sum::<usize>();
        (source.before, source.expected, names_end + 36, 30)
    } else {
        scalar::source()
    };
    before[20] = code;
    let mut expected = expected;
    expected[20] = code;

    let mut result = Case {
        before,
        expected,
        shape,
        data: shape.data(),
        roots: shape.roots(),
        physical_start,
        index_map: if shape == Shape::ScalarLargeHigh {
            16387
        } else {
            index_map
        },
        global_bitmap: if shape == Shape::ScalarLargeHigh {
            EXPLICIT_BYTES / PAGE - 1
        } else {
            32
        },
    };
    placement::apply(&mut result);
    result
}

pub fn over_cap_source(code: u8) -> Vec<u8> {
    let mut source = case(code, Shape::ScalarLargeLow).before;
    source.resize(EXPLICIT_BYTES + PAGE, 0xa7);
    source
}

pub fn row_span(bytes: &[u8], page: usize, slot: usize) -> Range<usize> {
    let base = page * PAGE;
    let directory = base + 14 + slot * 2;
    let start = usize::from(u16::from_le_bytes(
        bytes[directory..directory + 2].try_into().unwrap(),
    ));
    let end = if slot == 0 {
        PAGE
    } else {
        usize::from(u16::from_le_bytes(
            bytes[directory - 2..directory].try_into().unwrap(),
        ))
    };
    assert_eq!(start & 0xe000, 0);
    assert!(start < end && end <= PAGE);
    base + start..base + end
}
