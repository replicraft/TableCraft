#[path = "mutation/fixture.rs"]
mod fixture;
#[path = "ace16/int64_index_fixture.rs"]
mod int64_index_fixture;
#[path = "narrow_indexes/fixture.rs"]
mod narrow_fixture;
#[path = "ace16/scalar_fixture.rs"]
mod scalar_fixture;
mod support;

use std::{
    io::Cursor,
    ops::{
        Bound::{Excluded, Included, Unbounded},
        RangeBounds,
    },
};
use tablecraft_ace::{
    ByteIndexSelection, IndexProjectionSelection, IndexReadRequest, InspectError,
    Int16IndexSelection, Int32IndexSelection, read_table_index_with_request,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn database(code: u8, descending: bool, byte: bool, values: &[Option<i64>]) -> Vec<u8> {
    let mut bytes = narrow_fixture::database(code, !byte, descending, values, 0);
    if code == 1 {
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    }
    bytes
}

fn read(
    bytes: &[u8],
    selection: IndexProjectionSelection<'_>,
    projected: bool,
) -> Result<tablecraft_ace::TableSnapshot, InspectError> {
    read_table_index_with_request(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "BigKey",
        IndexReadRequest {
            columns: projected.then_some(&[]),
            selection,
            maximum_rows: 16384,
        },
        &mut ResourceBudget::new(32 * 1024 * 1024, 16384, 1_000_000),
        &|| false,
    )
}

#[test]
fn byte_and_short_selections_preserve_signedness_bounds_duplicates_and_nulls() {
    for byte in [false, true] {
        let values = if byte {
            vec![
                None,
                Some(0),
                Some(1),
                Some(127),
                Some(128),
                Some(255),
                Some(1),
            ]
        } else {
            vec![
                None,
                Some(-32768),
                Some(-1),
                Some(0),
                Some(1),
                Some(32767),
                Some(1),
            ]
        };
        for code in [1, 2, 5, 6] {
            for descending in [false, true] {
                let bytes = database(code, descending, byte, &values);
                for (lower, upper) in [
                    (Unbounded, Unbounded),
                    (Included(0), Included(1)),
                    (Excluded(0), Included(1)),
                    (Included(1), Excluded(1)),
                    (Included(1), Included(0)),
                ] {
                    let selection = if byte {
                        IndexProjectionSelection::Byte(ByteIndexSelection::Range {
                            lower: lower.map(|v| u8::try_from(v).unwrap()),
                            upper: upper.map(|v| u8::try_from(v).unwrap()),
                        })
                    } else {
                        IndexProjectionSelection::Int16(Int16IndexSelection::Range {
                            lower: lower.map(|v| i16::try_from(v).unwrap()),
                            upper: upper.map(|v| i16::try_from(v).unwrap()),
                        })
                    };
                    let result = read(&bytes, selection, false).unwrap();
                    let mut expected = values
                        .iter()
                        .copied()
                        .enumerate()
                        .filter_map(|(slot, value)| value.map(|value| (value, slot as i32 + 1)))
                        .filter(|(value, _)| (lower, upper).contains(value))
                        .collect::<Vec<_>>();
                    expected.sort_by(|a, b| {
                        let order = a.0.cmp(&b.0);
                        (if descending { order.reverse() } else { order }).then(a.1.cmp(&b.1))
                    });
                    let actual = result
                        .rows()
                        .iter()
                        .map(|row| {
                            let Value::Int32(id) = row[0] else {
                                panic!("ID")
                            };
                            let value = match row[1] {
                                Value::Byte(value) => i64::from(value),
                                Value::Int16(value) => i64::from(value),
                                _ => panic!("typed integer"),
                            };
                            (value, id)
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(actual, expected);
                }
                let all = if byte {
                    IndexProjectionSelection::Byte(ByteIndexSelection::All)
                } else {
                    IndexProjectionSelection::Int16(Int16IndexSelection::All)
                };
                assert_eq!(read(&bytes, all, false).unwrap().rows().len(), values.len());
                let null = if byte {
                    IndexProjectionSelection::Byte(ByteIndexSelection::Null)
                } else {
                    IndexProjectionSelection::Int16(Int16IndexSelection::Null)
                };
                assert_eq!(read(&bytes, null, true).unwrap().rows(), [vec![]]);
                assert!(matches!(
                    read(
                        &bytes,
                        IndexProjectionSelection::Int32(Int32IndexSelection::All),
                        false
                    ),
                    Err(InspectError::Unsupported)
                ));
                let wrong = if byte {
                    IndexProjectionSelection::Int16(Int16IndexSelection::Null)
                } else {
                    IndexProjectionSelection::Byte(ByteIndexSelection::Null)
                };
                assert!(matches!(
                    read(&bytes, wrong, false),
                    Err(InspectError::Unsupported)
                ));
            }
        }
    }
}
