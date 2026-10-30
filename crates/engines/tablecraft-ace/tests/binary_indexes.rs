#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
use std::io::Cursor;
use std::ops::Bound::{Excluded, Included, Unbounded};
use tablecraft_ace::{
    BinaryIndexSelection, IndexProjection, IndexProjectionSelection, read_table_binary_index_range,
    read_table_index_projection,
};
use tablecraft_ace::{
    EditError, IndexCheckStatus, InspectError, Int32IndexSelection, Int32Update, NativeImage,
    WriteBlocker, check_table, read_table_index, read_table_index_range,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE_BYTES: usize = 4096;
const INDEX_PAGE: usize = 13;
const DATA_PAGE: usize = 12;
const ENTRY_START: usize = 480;
const MASK_START: usize = 27;
const ENTRY_BYTES: usize = 14;
const SCOPE: CatalogScope = CatalogScope::new(1);
fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn database(code: u8, descending: bool) -> Vec<u8> {
    let mut bytes = index_fixture::database(descending, false, false);
    bytes[20] = code;
    if code != 1 {
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    }
    bytes[10 * PAGE_BYTES + 75 + 25] = 9; // Keep the four-byte fixed scalar payload.
    let page = &mut bytes[INDEX_PAGE * PAGE_BYTES..(INDEX_PAGE + 1) * PAGE_BYTES];
    page.fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    support::put32(page, 4, 10);
    for (position, slot) in (if descending {
        vec![2, 1, 0]
    } else {
        vec![0, 1, 2]
    })
    .into_iter()
    .enumerate()
    {
        let mut entry = vec![if descending { 0x80 } else { 0x7f }];
        let mut segment = [0_u8; 9];
        segment[0] = ((slot + 1) * 10) as u8;
        segment[8] = 4;
        if descending {
            segment = segment.map(|byte| !byte);
        }
        entry.extend(segment);
        entry.extend(((DATA_PAGE as u32) << 8 | slot).to_be_bytes());
        let start = ENTRY_START + position * ENTRY_BYTES;
        page[start..start + ENTRY_BYTES].copy_from_slice(&entry);
        let end = (position + 1) * ENTRY_BYTES;
        page[MASK_START + end / 8] |= 1 << (end % 8);
    }
    support::put16(page, 2, (PAGE_BYTES - ENTRY_START - 3 * ENTRY_BYTES) as u16);
    bytes
}
fn read(bytes: &[u8]) -> Result<tablecraft_ace::TableSnapshot, InspectError> {
    read_table_index(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        "ScoreKey",
        &mut budget(),
        &|| false,
    )
}

#[test]
fn binary_keys_have_checked_native_order_and_exact_values_in_every_profile() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            let bytes = database(code, descending);
            let table = read(&bytes).unwrap();
            let expected = if descending {
                vec![3, 2, 1]
            } else {
                vec![1, 2, 3]
            };
            for (row, id) in table.rows().iter().zip(expected) {
                assert_eq!(
                    row,
                    &[
                        Value::Int32(id),
                        Value::Binary(vec![(id * 10) as u8, 0, 0, 0])
                    ]
                );
            }
            let report = check_table(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(report.indexes()[0].status(), IndexCheckStatus::Checked);
            assert!(matches!(
                read_table_index_range(
                    &mut Cursor::new(bytes),
                    SCOPE,
                    "Items",
                    "ScoreKey",
                    Int32IndexSelection::All,
                    &mut budget(),
                    &|| false
                ),
                Err(InspectError::Unsupported)
            ));
        }
    }
}

#[test]
fn padding_and_key_row_disagreement_fail_without_partial_results() {
    let mut padding = database(1, false);
    padding[INDEX_PAGE * PAGE_BYTES + ENTRY_START + 5] = 1;
    assert!(matches!(read(&padding), Err(InspectError::Malformed)));
    let mut mismatch = database(1, false);
    let slot = DATA_PAGE * PAGE_BYTES + 14;
    let start =
        usize::from(u16::from_le_bytes(mismatch[slot..slot + 2].try_into().unwrap()) & 0x1fff);
    mismatch[DATA_PAGE * PAGE_BYTES + start + 6] = 11;
    assert!(matches!(read(&mismatch), Err(InspectError::Malformed)));
}

#[test]
fn binary_read_qualification_does_not_enable_native_edits() {
    let bytes = database(1, false);
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        image.plan_int32_updates(
            "Items",
            "Id",
            &[Int32Update::new(0, 1, 4)],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    ));
}

#[test]
fn typed_binary_ranges_and_projections_filter_exact_rows_in_every_direction_and_profile() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            let bytes = database(code, descending);
            let exact = BinaryIndexSelection::Range {
                lower: Included(&[20, 0, 0, 0]),
                upper: Included(&[20, 0, 0, 0]),
            };
            let mut controls = ResourceBudget::new(32 * 1024 * 1024, 4, 1_000_000);
            let selected = read_table_binary_index_range(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                "ScoreKey",
                exact,
                &mut controls,
                &|| false,
            )
            .unwrap();
            assert_eq!(
                selected.rows(),
                &[vec![Value::Int32(2), Value::Binary(vec![20, 0, 0, 0])]]
            );
            let projected = read_table_index_projection(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                "ScoreKey",
                IndexProjection {
                    columns: &["Id"],
                    selection: IndexProjectionSelection::Binary(exact),
                },
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(projected.rows(), &[vec![Value::Int32(2)]]);
            for selection in [
                BinaryIndexSelection::Null,
                BinaryIndexSelection::Range {
                    lower: Excluded(&[30, 0, 0, 0]),
                    upper: Unbounded,
                },
                BinaryIndexSelection::Range {
                    lower: Included(&[30, 0, 0, 0]),
                    upper: Included(&[10, 0, 0, 0]),
                },
            ] {
                assert!(
                    read_table_binary_index_range(
                        &mut Cursor::new(&bytes),
                        SCOPE,
                        "Items",
                        "ScoreKey",
                        selection,
                        &mut budget(),
                        &|| false
                    )
                    .unwrap()
                    .rows()
                    .is_empty()
                );
            }
        }
    }
}

#[test]
fn binary_range_qualification_controls_and_key_validation_are_never_waived() {
    let numeric = index_fixture::database(false, false, false);
    for selection in [BinaryIndexSelection::All, BinaryIndexSelection::Null] {
        assert!(matches!(
            read_table_binary_index_range(
                &mut Cursor::new(&numeric),
                SCOPE,
                "Items",
                "ScoreKey",
                selection,
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Unsupported)
        ));
    }
    let bytes = database(1, false);
    assert!(matches!(
        read_table_binary_index_range(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            "ScoreKey",
            BinaryIndexSelection::All,
            &mut budget(),
            &|| true
        ),
        Err(InspectError::Cancelled)
    ));
    assert!(matches!(
        read_table_binary_index_range(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            "ScoreKey",
            BinaryIndexSelection::All,
            &mut ResourceBudget::new(0, 100, 1_000_000),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
    let overlong = vec![0; BinaryIndexSelection::MAX_BOUND_BYTES + 1];
    assert!(matches!(
        read_table_binary_index_range(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            "ScoreKey",
            BinaryIndexSelection::Range {
                lower: Included(&overlong),
                upper: Unbounded
            },
            &mut budget(),
            &|| false
        ),
        Err(InspectError::StructureLimit)
    ));
    let mut broken = bytes;
    broken[INDEX_PAGE * PAGE_BYTES + ENTRY_START + ENTRY_BYTES + 5] = 1;
    let selected = BinaryIndexSelection::Range {
        lower: Included(&[20, 0, 0, 0]),
        upper: Included(&[20, 0, 0, 0]),
    };
    assert!(matches!(
        read_table_index_projection(
            &mut Cursor::new(broken),
            SCOPE,
            "Items",
            "ScoreKey",
            IndexProjection {
                columns: &["Id"],
                selection: IndexProjectionSelection::Binary(selected)
            },
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Malformed)
    ));
}
