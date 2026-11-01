//! Source-only cases and independent original-request framing.
use sha2::{Digest, Sha256};
use tablecraft_ace::{NativeCompleteInsertIntent, RowInsert};
use tablecraft_testkit::access::canonical_ordinary_wide_insert::{self as sources, Case};

pub(super) struct View {
    pub case: Case,
    pub source: sources::Source,
}

pub(super) fn cases() -> Vec<View> {
    let mut views = Vec::new();
    for case in sources::CASES {
        for header in [5_u32, 6] {
            let mut source = sources::source(case);
            source.before[20..24].copy_from_slice(&header.to_le_bytes());
            source.after[20..24].copy_from_slice(&header.to_le_bytes());
            views.push(View { case, source });
        }
    }
    views
}

pub(super) fn requests(view: &View) -> Vec<RowInsert> {
    vec![RowInsert::new(view.source.values.clone())]
}

pub(super) fn wire(view: &View) -> Vec<u8> {
    let cells: Vec<(u8, Vec<u8>)> = match view.case {
        Case::FixedNull => {
            let mut cells = vec![(1, vec![0])];
            cells.extend((0..9).map(|_| (0, vec![])));
            cells
        }
        Case::DecimalExact => vec![
            (2, vec![99, 0, 0, 0]),
            (6, b"new".to_vec()),
            (7, vec![199, 0, 255]),
            (
                13,
                vec![
                    199, 207, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    4,
                ],
            ),
            (1, vec![1]),
        ],
        Case::DecimalMax => {
            let mut decimal = 9_999_999_999_999_999_999_999_999_999_i128
                .to_le_bytes()
                .to_vec();
            decimal.push(4);
            vec![
                (2, vec![100, 0, 0, 0]),
                (0, vec![]),
                (0, vec![]),
                (13, decimal),
                (1, vec![0]),
            ]
        }
        Case::DecimalNull => vec![
            (2, vec![101, 0, 0, 0]),
            (0, vec![]),
            (0, vec![]),
            (0, vec![]),
            (1, vec![0]),
        ],
        _ => {
            let (single, double, date) = match view.case {
                Case::FixedSubnormal => (1_u32, 0x000f_ffff_ffff_ffff_u64, 0_u64),
                Case::FixedSpecial => (0x7f81_2345, 0xfff8_1234_5678_9abc, 0),
                _ => (0x8000_0000, 0x8000_0000_0000_0000, 0x8000_0000_0000_0000),
            };
            vec![
                (1, vec![1]),
                (8, vec![255]),
                (9, vec![0, 128]),
                (2, vec![17, 0, 0, 0]),
                (3, (-7_i64).to_le_bytes().to_vec()),
                (4, (-10001_i64).to_le_bytes().to_vec()),
                (10, single.to_le_bytes().to_vec()),
                (11, double.to_le_bytes().to_vec()),
                (5, date.to_le_bytes().to_vec()),
                (
                    12,
                    vec![
                        0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255,
                    ],
                ),
            ]
        }
    };
    let row_bytes = 8 + cells
        .iter()
        .map(|(_, bytes)| 8 + bytes.len())
        .sum::<usize>();
    let total = 73 + row_bytes;
    let mut wire = Vec::from(*b"TCNA0004");
    wire.extend_from_slice(&(total as u32).to_le_bytes());
    wire.extend_from_slice(&[4, 0, 0, 0]);
    wire.extend_from_slice(&(view.source.before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((view.source.before.len() as u64).to_le_bytes());
    hash.update(&view.source.before);
    wire.extend_from_slice(&hash.finalize());
    for word in [5_u32, 1, 0] {
        wire.extend_from_slice(&word.to_le_bytes());
    }
    wire.extend_from_slice(b"Items");
    wire.extend_from_slice(&(row_bytes as u32).to_le_bytes());
    wire.extend_from_slice(&(cells.len() as u32).to_le_bytes());
    for (tag, bytes) in cells {
        wire.extend_from_slice(&[tag, 0, 0, 0]);
        wire.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        wire.extend_from_slice(&bytes);
    }
    assert_eq!(wire.len(), total);
    wire
}

pub(super) fn intent(view: &View) -> NativeCompleteInsertIntent {
    NativeCompleteInsertIntent::decode(&wire(view), &mut super::budget(), &|| false).unwrap()
}
