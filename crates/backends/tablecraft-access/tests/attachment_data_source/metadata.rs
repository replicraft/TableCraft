use super::*;

const PAYLOAD_PAGE: usize = 23;

fn external() -> Vec<u8> {
    let mut descriptor = (0x40000000 | attachment_fixture::PAYLOAD.len() as u32)
        .to_le_bytes()
        .to_vec();
    descriptor.extend(((PAYLOAD_PAGE as u32) << 8).to_le_bytes());
    descriptor.extend([0; 4]);
    let mut bytes = attachment_fixture::with_data_fields(&descriptor, &[]);
    bytes.resize((PAYLOAD_PAGE + 1) * PAGE_BYTES, 0);
    let mut page = support::data_page(0, &[(0, attachment_fixture::PAYLOAD.to_vec())]);
    page[4..8].copy_from_slice(b"LVAL");
    bytes[PAYLOAD_PAGE * PAGE_BYTES..].copy_from_slice(&page);
    bytes
}

#[test]
fn metadata_cache_omits_payloads_and_failed_payload_reads_leave_it_available() {
    let payload = Rc::new(PayloadWatch {
        page: PAYLOAD_PAGE,
        forbidden: Cell::new(true),
        reads: Cell::new(0),
    });
    let forbid_records = Rc::new(Cell::new(true));
    let forbid_children = Rc::new(Cell::new(false));
    let child_reads = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        Watching {
            bytes: Cursor::new(external()),
            forbid_records: forbid_records.clone(),
            forbid_children: forbid_children.clone(),
            reads: child_reads.clone(),
            payload: Some(payload.clone()),
        },
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_decoded_attachments();
    let parsed = parse_query("SELECT Score.FileName FROM Items", ParseLimits::default()).unwrap();
    prepare(parsed.query(), &source).unwrap();
    assert_eq!(child_reads.get(), 0);
    assert_eq!(payload.reads.get(), 0);
    let presence = parse_query(
        "SELECT Count(*) FROM Items WHERE Score IS NOT NULL",
        ParseLimits::default(),
    )
    .unwrap();
    prepare(presence.query(), &source).unwrap();
    assert_eq!(child_reads.get(), 0);
    assert_eq!(payload.reads.get(), 0);

    forbid_records.set(false);
    assert_eq!(
        execute(
            &source,
            "SELECT Count(*) FROM Items WHERE Score IS NOT NULL"
        )
        .unwrap(),
        [vec![Value::Int64(2)]]
    );
    assert_eq!(payload.reads.get(), 0);
    let count = "SELECT Count(Score.FileName) FROM Items";
    assert_eq!(execute(&source, count).unwrap(), [vec![Value::Int64(2)]]);
    assert!(child_reads.get() > 0);
    assert_eq!(payload.reads.get(), 0);
    assert_eq!(
        execute(
            &source,
            "SELECT Count(*) FROM Items WHERE Score IS NOT NULL"
        )
        .unwrap(),
        [vec![Value::Int64(2)]]
    );
    assert_eq!(
        execute(&source, "SELECT Count(*) FROM Items WHERE Score IS NULL").unwrap(),
        [vec![Value::Int64(1)]]
    );
    assert_eq!(payload.reads.get(), 0);
    forbid_children.set(true);
    assert_eq!(execute(&source, count).unwrap(), [vec![Value::Int64(2)]]);

    forbid_children.set(false);
    assert!(execute(&source, "SELECT Score.FileData FROM Items").is_err());
    assert_eq!(payload.reads.get(), 1);
    forbid_children.set(true);
    assert_eq!(execute(&source, count).unwrap(), [vec![Value::Int64(2)]]);

    payload.forbidden.set(false);
    forbid_children.set(false);
    let rows = execute(&source, "SELECT Score.FileName,Score.FileData FROM Items").unwrap();
    assert_eq!(
        rows[0],
        [
            Value::Text("../é🦀\0.bin".into()),
            Value::Binary(vec![0, 1, 2, 255])
        ]
    );
    assert_eq!(rows.len(), 4);
    assert_eq!(payload.reads.get(), 2);

    payload.forbidden.set(true);
    forbid_children.set(true);
    assert_eq!(execute(&source, count).unwrap(), [vec![Value::Int64(2)]]);
    assert_eq!(payload.reads.get(), 2);
}
