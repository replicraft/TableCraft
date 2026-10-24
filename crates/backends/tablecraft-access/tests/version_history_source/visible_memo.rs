use super::{
    CatalogScope, Cell, Cursor, NativeSource, PAGE_BYTES, Rc, Source, Value, Watching, budget,
    execute, fixture,
};

const OWNER_PAGE: usize = 10;
const FIELD_START: usize = 63;
const FIELD_BYTES: usize = 25;
const MEMO_POSITION: usize = 2;
const FLAGS: usize = 15;
const EXTENDED_FLAGS: usize = 16;

#[test]
fn hyperlink_current_memo_history_keeps_metadata_only_binding_and_unknown_field_refusal() {
    for extended in [0, 1] {
        let mut bytes = fixture::database();
        let column = OWNER_PAGE * PAGE_BYTES + FIELD_START + MEMO_POSITION * FIELD_BYTES;
        bytes[column + FLAGS] = 0x82;
        bytes[column + EXTENDED_FLAGS] = extended;
        let forbid_records = Rc::new(Cell::new(true));
        let source = NativeSource::new(
            Watching {
                bytes: Cursor::new(bytes),
                forbid_records: forbid_records.clone(),
                forbid_children: Rc::new(Cell::new(false)),
                child_reads: Rc::new(Cell::new(0)),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_collections();
        assert_eq!(source.collection_members("Items").unwrap().len(), 2);
        forbid_records.set(false);
        assert_eq!(
            execute(&source, "SELECT Count(Score.Notes) AS N FROM Items").unwrap(),
            [vec![Value::Int64(3)]]
        );
        assert!(execute(&source, "SELECT Score.FileData FROM Items").is_err());
    }
}
