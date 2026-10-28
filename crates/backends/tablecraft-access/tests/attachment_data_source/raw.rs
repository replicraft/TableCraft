use super::*;

#[test]
fn default_file_data_is_stored_binary_without_decoder_or_metadata_reads() {
    for code in [2, 5, 6] {
        // The final present value is an empty invalid wrapper. Stored Binary
        // access must retain it; only explicit decoding rejects its contents.
        let mut bytes = attachment_fixture::database();
        bytes[20] = code;
        let forbid_records = Rc::new(Cell::new(true));
        let forbid_children = Rc::new(Cell::new(false));
        let reads = Rc::new(Cell::new(0));
        let source = NativeSource::new(
            Watching {
                bytes: Cursor::new(bytes),
                forbid_records: forbid_records.clone(),
                forbid_children: forbid_children.clone(),
                reads: reads.clone(),
                payload: None,
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_collections();
        let members = source.collection_members("Items").unwrap();
        let index = members
            .iter()
            .position(|member| member.column().name() == "FileData")
            .unwrap();
        assert_eq!(members[index].column().data_type(), ScalarType::Binary);
        assert!(members[index].column().nullable());
        assert_eq!(
            source.collection_member_text_collations("Items").unwrap()[index],
            None
        );
        let parsed =
            parse_query("SELECT Score.FileData FROM Items", ParseLimits::default()).unwrap();
        prepare(parsed.query(), &source).unwrap();
        assert_eq!(reads.get(), 0);

        forbid_records.set(false);
        assert_eq!(
            execute(
                &source,
                "SELECT Id,Score.FileType,Score.FileData FROM Items"
            )
            .unwrap(),
            [
                vec![
                    Value::Int32(1),
                    Value::Text("field/type".into()),
                    Value::Binary(attachment_fixture::PAYLOAD.to_vec())
                ],
                vec![Value::Int32(1), Value::Null, Value::Null],
                vec![
                    Value::Int32(2),
                    Value::Text("".into()),
                    Value::Binary(vec![])
                ],
                vec![Value::Int32(3), Value::Null, Value::Null],
            ]
        );
        assert!(reads.get() > 0);
        forbid_children.set(true);
        assert_eq!(
            execute(&source, "SELECT Count(Score.FileData) FROM Items").unwrap(),
            [vec![Value::Int64(2)]]
        );
        assert_eq!(
            execute(
                &source,
                "SELECT Id FROM Items WHERE Score.FileData IS NULL ORDER BY Id"
            )
            .unwrap(),
            [vec![Value::Int32(1)]]
        );
        assert_eq!(
            execute(
                &source,
                "SELECT Id,Score.FileData FROM Items WHERE Score.FileData IS NOT NULL ORDER BY Id"
            )
            .unwrap(),
            [
                vec![
                    Value::Int32(1),
                    Value::Binary(attachment_fixture::PAYLOAD.to_vec())
                ],
                vec![Value::Int32(2), Value::Binary(vec![])]
            ]
        );

        // A full encoded snapshot is retained across explicit decoder activation.
        // The unsupported last wrapper fails decoding without rereading records;
        // complete metadata and ordinary parent counts remain available.
        let source = source.with_decoded_attachments();
        assert!(execute(&source, "SELECT TOP 1 Score.FileData FROM Items").is_err());
        assert_eq!(
            execute(&source, "SELECT Count(Score.FileName) FROM Items").unwrap(),
            [vec![Value::Int64(2)]]
        );
        assert_eq!(
            execute(&source, "SELECT Count(*) FROM Items").unwrap(),
            [vec![Value::Int64(3)]]
        );
    }
}

#[test]
fn default_encoded_data_keeps_payload_pruning_and_failure_isolation() {
    let payload = Rc::new(PayloadWatch {
        page: 23,
        forbidden: Cell::new(true),
        reads: Cell::new(0),
    });
    let source = NativeSource::new(
        Watching {
            bytes: Cursor::new(metadata::external()),
            forbid_records: Rc::new(Cell::new(false)),
            forbid_children: Rc::new(Cell::new(false)),
            reads: Rc::new(Cell::new(0)),
            payload: Some(payload.clone()),
        },
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections();
    assert_eq!(
        execute(&source, "SELECT Count(Score.FileName) FROM Items").unwrap(),
        [vec![Value::Int64(2)]]
    );
    assert_eq!(payload.reads.get(), 0);
    assert!(execute(&source, "SELECT Score.FileData FROM Items").is_err());
    assert_eq!(payload.reads.get(), 1);
    assert_eq!(
        execute(&source, "SELECT Count(Score.FileName) FROM Items").unwrap(),
        [vec![Value::Int64(2)]]
    );
    payload.forbidden.set(false);
    let rows = execute(&source, "SELECT Score.FileData FROM Items").unwrap();
    assert_eq!(
        rows[0],
        [Value::Binary(attachment_fixture::PAYLOAD.to_vec())]
    );
    assert_eq!(payload.reads.get(), 2);
    payload.forbidden.set(true);
    assert_eq!(
        execute(&source, "SELECT Count(Score.FileData) FROM Items").unwrap(),
        [vec![Value::Int64(1)]]
    );
    assert_eq!(payload.reads.get(), 2);
}

#[test]
fn stored_data_keeps_selected_index_cancellation_and_native_budget_failures() {
    let bytes = attachment_fixture::with_payloads(attachment_fixture::PAYLOAD, EMPTY_FILE);
    let source = NativeSource::new(
        Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections()
    .with_index("Items", "Missing")
    .unwrap();
    assert_eq!(
        execute(&source, "SELECT Score.FileData FROM Items").err(),
        Some(QueryError::UnknownName)
    );
    let member = source
        .collection_members("Items")
        .unwrap()
        .iter()
        .position(|member| member.column().name() == "FileData")
        .unwrap();
    let mut callbacks = 0;
    assert_eq!(
        source.scan_collection_members("Items", &[], &[], &[member], &mut |_, _| {
            callbacks += 1;
            Ok(())
        }),
        Err(QueryError::UnknownName)
    );
    assert_eq!(callbacks, 0);

    let armed = Cell::new(false);
    let cancelled = || armed.get();
    let source = NativeSource::new(
        Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        budget(),
        &cancelled,
    )
    .unwrap()
    .with_collections();
    source.collection_members("Items").unwrap();
    armed.set(true);
    assert_eq!(
        execute(&source, "SELECT Score.FileData FROM Items").err(),
        Some(QueryError::Cancelled)
    );
    armed.set(false);
    assert_eq!(
        execute(&source, "SELECT Count(Score.FileData) FROM Items").unwrap(),
        [vec![Value::Int64(2)]]
    );

    let mut exhausted = false;
    for ceiling in [64 * 1024, 128 * 1024, 256 * 1024] {
        if let Ok(source) = NativeSource::new(
            Cursor::new(bytes.clone()),
            CatalogScope::new(1),
            ResourceBudget::new(ceiling, 16384, 1_000_000),
            &|| false,
        ) {
            exhausted |= matches!(
                execute(
                    &source.with_collections(),
                    "SELECT Score.FileData FROM Items"
                ),
                Err(QueryError::Limit(_))
            );
        }
    }
    assert!(exhausted);
}

#[test]
fn unknown_wrapper_bytes_remain_stored_values_until_decoding_is_requested() {
    let mut unknown = attachment_fixture::PAYLOAD.to_vec();
    unknown[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    let source = NativeSource::new(
        Cursor::new(attachment_fixture::with_payloads(&unknown, EMPTY_FILE)),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections();
    let rows = execute(&source, "SELECT Score.FileData FROM Items").unwrap();
    assert_eq!(rows[0], [Value::Binary(unknown)]);

    let source = source.with_decoded_attachments();
    assert_eq!(
        execute(&source, "SELECT Score.FileData FROM Items").err(),
        Some(QueryError::Unsupported)
    );
}
