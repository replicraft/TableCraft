use super::*;

fn variable_references(mut bytes: Vec<u8>) -> Vec<u8> {
    const DEFINITION: usize = 20 * 4096;
    const DIRECTORY: usize = 14;
    const FIELD_START: usize = 63;
    const FIELD_BYTES: usize = 25;

    let original = &bytes[CHILD_PAGE * PAGE_BYTES..(CHILD_PAGE + 1) * PAGE_BYTES];
    let mut rows = Vec::new();
    for slot in 0..3 {
        let at = DIRECTORY + slot * 2;
        let start = u16::from_le_bytes(original[at..at + 2].try_into().unwrap()) as usize;
        let end = if slot == 0 {
            PAGE_BYTES
        } else {
            u16::from_le_bytes(original[at - 2..at].try_into().unwrap()) as usize
        };
        let old = &original[start..end];
        let mask = old.len() - 1;
        assert_eq!(
            u16::from_le_bytes(old[mask - 2..mask].try_into().unwrap()),
            4
        );
        let offsets: Vec<usize> = (0..5)
            .map(|ordinal| {
                let at = mask - 4 - ordinal * 2;
                u16::from_le_bytes(old[at..at + 2].try_into().unwrap()) as usize
            })
            .collect();

        let mut row = vec![8, 0];
        row.extend_from_slice(&old[10..22]);
        let mut positions = vec![row.len()];
        for payload in [
            &old[2..6],
            &old[6..10],
            &old[offsets[0]..offsets[1]],
            &old[offsets[1]..offsets[2]],
            &old[offsets[2]..offsets[3]],
            &old[offsets[3]..offsets[4]],
        ] {
            row.extend_from_slice(payload);
            positions.push(row.len());
        }
        for position in positions.into_iter().rev() {
            row.extend_from_slice(&(position as u16).to_le_bytes());
        }
        row.extend_from_slice(&6_u16.to_le_bytes());
        row.push(old[mask]);
        rows.push((0, row));
    }

    let fields = [
        ("Owner", 4, 0, 4),
        ("ChildID", 4, 0, 4),
        ("FileURL", 12, 0, 0),
        ("FileName", 10, 0, 510),
        ("FileType", 10, 0, 510),
        ("FileData", 11, 0, 0),
        ("FileTimeStamp", 8, 0, 8),
        ("FileFlags", 4, 8, 4),
    ];
    let mut definition = attachment_fixture::definition(&fields, 3, 21 << 8, false);
    support::put16(&mut definition, 43, 6);
    for position in 0..8 {
        let at = FIELD_START + position * FIELD_BYTES;
        definition[at + 16] = if position < 2 { 0 } else { 0x10 };
        if position < 6 {
            definition[at + 15] = 2;
            support::put16(&mut definition, at + 7, position as u16);
        }
    }
    definition[FIELD_START + 16] = 8;
    definition[FIELD_START + FIELD_BYTES + 15] = 6;
    definition[FIELD_START + FIELD_BYTES + 16] = 4;
    bytes[DEFINITION..DEFINITION + PAGE_BYTES].copy_from_slice(&definition);
    bytes[CHILD_PAGE * PAGE_BYTES..(CHILD_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(20, &rows));
    bytes
}

#[test]
fn qualified_modern_members_keep_stored_and_decoded_cache_authority() {
    for code in [2, 5, 6] {
        let mut bytes = variable_references(attachment_fixture::with_payloads(
            attachment_fixture::PAYLOAD,
            EMPTY_FILE,
        ));
        bytes[20] = code;
        let forbid_records = Rc::new(Cell::new(true));
        let forbid_children = Rc::new(Cell::new(false));
        let child_reads = Rc::new(Cell::new(0));
        let source = NativeSource::new(
            Watching {
                bytes: Cursor::new(bytes),
                forbid_records: Rc::clone(&forbid_records),
                forbid_children: Rc::clone(&forbid_children),
                reads: Rc::clone(&child_reads),
                payload: None,
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_collections();
        let members = source.collection_members("Items").unwrap();
        assert_eq!(members.len(), 6);
        assert_eq!(
            members
                .iter()
                .find(|member| member.column().name() == "FileData")
                .unwrap()
                .column()
                .data_type(),
            ScalarType::Binary
        );
        let parsed = parse_query(
            "SELECT Id,Score.FileData FROM Items",
            ParseLimits::default(),
        )
        .unwrap();
        prepare(parsed.query(), &source).unwrap();
        assert_eq!(child_reads.get(), 0);

        forbid_records.set(false);
        let rows = execute(
            &source,
            "SELECT Id,Score.FileName,Score.FileData FROM Items",
        )
        .unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows[0],
            [
                Value::Int32(1),
                Value::Text("../é🦀\0.bin".into()),
                Value::Binary(attachment_fixture::PAYLOAD.to_vec()),
            ]
        );
        assert!(child_reads.get() > 0);
        forbid_children.set(true);
        let source = source.with_decoded_attachments();
        assert_eq!(
            execute(&source, "SELECT Id,Score.FileData FROM Items").unwrap(),
            [
                vec![Value::Int32(1), Value::Binary(vec![0, 1, 2, 255])],
                vec![Value::Int32(1), Value::Null],
                vec![Value::Int32(2), Value::Binary(vec![])],
                vec![Value::Int32(3), Value::Null],
            ]
        );
        assert_eq!(
            execute(&source, "SELECT Count(Score.FileName) FROM Items").unwrap(),
            [vec![Value::Int64(2)]]
        );
    }
}

#[test]
fn modern_metadata_omits_payload_reads_and_survives_failed_payload_publication() {
    let mut bytes = variable_references(metadata::external());
    bytes[20] = 6;
    let payload = Rc::new(PayloadWatch {
        page: 23,
        forbidden: Cell::new(true),
        reads: Cell::new(0),
    });
    let forbid_records = Rc::new(Cell::new(true));
    let forbid_children = Rc::new(Cell::new(false));
    let child_reads = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        Watching {
            bytes: Cursor::new(bytes),
            forbid_records: Rc::clone(&forbid_records),
            forbid_children: Rc::clone(&forbid_children),
            reads: Rc::clone(&child_reads),
            payload: Some(Rc::clone(&payload)),
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

    forbid_records.set(false);
    let count = "SELECT Count(Score.FileName) FROM Items";
    assert_eq!(execute(&source, count).unwrap(), [vec![Value::Int64(2)]]);
    assert!(child_reads.get() > 0);
    assert_eq!(payload.reads.get(), 0);
    forbid_children.set(true);
    assert_eq!(execute(&source, count).unwrap(), [vec![Value::Int64(2)]]);

    forbid_children.set(false);
    assert!(execute(&source, "SELECT Score.FileData FROM Items").is_err());
    assert_eq!(payload.reads.get(), 1);
    forbid_children.set(true);
    assert_eq!(execute(&source, count).unwrap(), [vec![Value::Int64(2)]]);
    assert_eq!(payload.reads.get(), 1);
}
