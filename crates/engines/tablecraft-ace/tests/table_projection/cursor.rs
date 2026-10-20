use super::*;
use tablecraft_ace::TableCursor;
#[path = "../large_maps/fixture.rs"]
mod map_fixture;
#[path = "../support/page_reads.rs"]
mod page_reads;

const DATA_PAGE: usize = 12;
const SCOPE: CatalogScope = CatalogScope::new(1);

#[test]
fn projected_batches_preserve_native_order_and_match_full_projection() {
    let bytes = fixture::database();
    for names in [&["score", "ID"][..], &["Score"][..], &[][..]] {
        let snapshot = read_table_projection(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            names,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut input = Cursor::new(&bytes);
        let mut cursor =
            TableCursor::open_projection(&mut input, SCOPE, "Items", names, &mut budget(), &|| {
                false
            })
            .unwrap();
        assert!(cursor.columns() == snapshot.columns());
        for (offset, expected) in snapshot.rows().iter().enumerate() {
            let batch = cursor.next_page(1, &mut budget(), &|| false).unwrap();
            assert_eq!(batch.offset(), offset as u32);
            assert_eq!(batch.rows(), std::slice::from_ref(expected));
        }
        assert!(
            cursor
                .next_page(1, &mut budget(), &|| false)
                .unwrap()
                .reached_end()
        );
    }
}

#[test]
fn unsupported_unselected_columns_remain_visible_without_decoding_or_null_substitution() {
    let mut bytes = fixture::database();
    bytes[OWNER_PAGE * PAGE_BYTES + FIRST_FIELD + FIELD_BYTES] = UNKNOWN_TYPE;
    let mut input = Cursor::new(&bytes);
    let mut cursor =
        TableCursor::open_projection(&mut input, SCOPE, "Items", &["Id"], &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(cursor.schema().columns().len(), 2);
    assert_eq!(cursor.columns().len(), 1);
    assert_eq!(
        cursor
            .next_page(2, &mut budget(), &|| false)
            .unwrap()
            .rows(),
        &[vec![Value::Int32(1)], vec![Value::Int32(2)]]
    );
    assert_eq!(
        cursor
            .next_page(2, &mut budget(), &|| false)
            .unwrap()
            .rows(),
        &[vec![Value::Int32(3)]]
    );
    let mut input = Cursor::new(&bytes);
    assert!(matches!(
        TableCursor::open(&mut input, SCOPE, "Items", &mut budget(), &|| false),
        Err(InspectError::Unsupported)
    ));
    for names in [
        &["Score"][..],
        &["Id", "Id"][..],
        &["ID", "id"][..],
        &["missing"][..],
    ] {
        let mut input = page_reads::PageReads::new(bytes.clone());
        assert!(
            TableCursor::open_projection(&mut input, SCOPE, "Items", names, &mut budget(), &|| {
                false
            })
            .is_err()
        );
        assert_eq!(input.count(DATA_PAGE as u64), 0);
    }
    support::put32(&mut bytes, OWNER_PAGE * PAGE_BYTES + 16, 0);
    assert!(matches!(
        TableCursor::open_projection(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &["Score"],
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
}

#[test]
fn empty_projection_still_validates_fetched_row_framing_and_closes_on_error() {
    let mut bytes = fixture::database();
    let slot = DATA_PAGE * PAGE_BYTES + 14;
    let start = usize::from(u16::from_le_bytes(bytes[slot..slot + 2].try_into().unwrap()) & 0x1fff);
    support::put16(&mut bytes, DATA_PAGE * PAGE_BYTES + start, u16::MAX);
    for names in [&["Id"][..], &[][..]] {
        let mut input = Cursor::new(&bytes);
        let mut cursor =
            TableCursor::open_projection(&mut input, SCOPE, "Items", names, &mut budget(), &|| {
                false
            })
            .unwrap();
        assert!(cursor.next_page(1, &mut budget(), &|| false).is_err());
        assert!(cursor.is_failed());
        assert_eq!(cursor.position(), 0);
    }
}

#[test]
fn large_mixed_tables_resume_all_selected_rows_in_every_read_profile() {
    for code in [1, 2, 5, 6] {
        let mut bytes = map_fixture::database(6000, true, false);
        bytes[20] = code;
        if code != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        bytes[OWNER_PAGE * PAGE_BYTES + FIRST_FIELD + FIELD_BYTES] = UNKNOWN_TYPE;
        let mut input = Cursor::new(bytes);
        let mut cursor = TableCursor::open_projection(
            &mut input,
            SCOPE,
            "Items",
            &["Id"],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut count = 0;
        loop {
            let batch = cursor.next_page(250, &mut budget(), &|| false).unwrap();
            for row in batch.rows() {
                count += 1;
                assert_eq!(row, &[Value::Int32(count)]);
            }
            if batch.reached_end() {
                break;
            }
        }
        assert_eq!(count, 6000);
    }
}
