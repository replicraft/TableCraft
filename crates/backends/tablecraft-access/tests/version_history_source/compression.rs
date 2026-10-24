use super::{
    CHILD_PAGE, CatalogScope, Cell, Cursor, ExecutionOptions, NativeSource, PAGE_BYTES,
    ParseLimits, Rc, Source, Value, Watching, budget, execute, fixture, parse_query, prepare,
};

const OWNER_PAGE: usize = 10;
const FIELD_START: usize = 63;
const FIELD_BYTES: usize = 25;
const VISIBLE_MEMO: usize = 2;
const EXTENDED_FLAGS: usize = 16;

#[test]
fn compressed_current_memo_history_binds_without_rows_and_reuses_complete_cache() {
    for profile in [2, 5, 6] {
        let mut bytes = fixture::database();
        bytes[20] = profile;
        bytes
            [OWNER_PAGE * PAGE_BYTES + FIELD_START + VISIBLE_MEMO * FIELD_BYTES + EXTENDED_FLAGS] =
            1;
        let forbid_records = Rc::new(Cell::new(true));
        let forbid_children = Rc::new(Cell::new(false));
        let child_reads = Rc::new(Cell::new(0));
        let source = NativeSource::new(
            Watching {
                bytes: Cursor::new(bytes),
                forbid_records: forbid_records.clone(),
                forbid_children: forbid_children.clone(),
                child_reads: child_reads.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_collections();
        let parsed = parse_query(
            "SELECT Id,Notes,Score.Notes FROM Items",
            ParseLimits::default(),
        )
        .unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.collection_members("Items").unwrap().len(), 2);
        assert_eq!(child_reads.get(), 0);

        forbid_records.set(false);
        let result = plan
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(result.rows().len(), 5);
        assert_eq!(
            result.rows()[0],
            [
                Value::Int32(1),
                Value::Null,
                Value::Text("é🦀\0older".into()),
            ]
        );
        assert!(child_reads.get() > 0);
        forbid_children.set(true);
        assert_eq!(
            execute(&source, "SELECT Count(Score.Notes) AS N FROM Items").unwrap(),
            [vec![Value::Int64(3)]]
        );
    }
}

#[test]
fn compressed_visible_memo_does_not_hide_a_late_orphan_under_top() {
    let mut bytes = fixture::database();
    bytes[OWNER_PAGE * PAGE_BYTES + FIELD_START + VISIBLE_MEMO * FIELD_BYTES + EXTENDED_FLAGS] = 1;
    let page = CHILD_PAGE * PAGE_BYTES;
    let start = page
        + usize::from(u16::from_le_bytes(
            bytes[page + 20..page + 22].try_into().unwrap(),
        ));
    bytes[start + 2..start + 6].copy_from_slice(&999_i32.to_le_bytes());
    let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
        false
    })
    .unwrap()
    .with_collections();
    assert!(execute(&source, "SELECT TOP 1 Score.Notes FROM Items").is_err());
}
