//! Real typed parent reads use a complete key directory, never a payload prefix.
use super::*;
use tablecraft_ace::{ComplexReferenceViolation, MultivalueReader, TableSnapshotLimits};
use tablecraft_core::limits::Resource;
use tablecraft_testkit::support::ace::mutable_source::MutableSource;

#[test]
fn literal_parent_lists_preserve_order_nulls_and_ready_range_checks() {
    for code in [2, 5, 6] {
        let mut bytes = links_fixture::populated();
        bytes[20] = code;
        for (slot, identity) in [i32::MIN, -1, 0].into_iter().enumerate() {
            let word = CHILD_PAGE * PAGE_BYTES + 14 + 2 * slot;
            let start =
                usize::from(u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap()) & 0x1fff);
            put32(
                &mut bytes,
                CHILD_PAGE * PAGE_BYTES + start + 10,
                identity as u32,
            );
        }
        let mut source = Cursor::new(&bytes);
        let mut allowance = budget();
        let mut reader = MultivalueReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .unwrap();
        assert_eq!(reader.parents().len(), 3);
        assert_eq!(reader.schema().owner().name(), "Items");
        assert_eq!(reader.value_column().data_type(), ScalarType::Int32);
        assert!(matches!(
            reader.read_parent(3),
            Err(InspectError::InvalidPageRange)
        ));
        assert!(!reader.is_failed());
        for (parent, expected) in [
            (0, vec![Value::Int32(100), Value::Int32(101)]),
            (1, vec![Value::Int32(200)]),
            (2, vec![]),
        ] {
            let values = reader.read_parent(parent).unwrap();
            assert_eq!(values.values(), expected);
            assert_eq!(values.key(), Some((parent as i32 + 1) * 10));
            assert_eq!(values.row_reference().page_number(), 12);
            assert_eq!(values.row_reference().slot_number(), parent as u8);
        }
    }
    let mut bytes = links_fixture::populated();
    rows(
        &mut bytes,
        &[Some(i32::MIN), Some(i32::MIN), None, Some(i32::MAX)],
    );
    let mut source = Cursor::new(&bytes);
    let mut allowance = budget();
    let result = {
        let mut reader = MultivalueReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .unwrap();
        reader.read_parent(0).unwrap()
    };
    assert_eq!(
        result.values(),
        [
            Value::Int32(i32::MIN),
            Value::Int32(i32::MIN),
            Value::Null,
            Value::Int32(i32::MAX)
        ]
    );
    let complete = read(&bytes).unwrap();
    assert_eq!(
        complete.values(0).unwrap().collect::<Vec<_>>(),
        result.values().iter().collect::<Vec<_>>()
    );
}

#[test]
fn complete_directory_rejects_unrequested_key_corruption_and_every_parent_limit() {
    for variant in 0..4 {
        let mut bytes = links_fixture::populated();
        let start = usize::from(
            u16::from_le_bytes(
                bytes[CHILD_PAGE * PAGE_BYTES + 18..CHILD_PAGE * PAGE_BYTES + 20]
                    .try_into()
                    .unwrap(),
            ) & 0x1fff,
        );
        match variant {
            0 => put32(&mut bytes, CHILD_PAGE * PAGE_BYTES + start + 2, 99),
            1 => put32(&mut bytes, CHILD_PAGE * PAGE_BYTES + start + 10, 1),
            2 => bytes[CHILD_PAGE * PAGE_BYTES + start + 14] = 3,
            _ => put32(&mut bytes, FLAT_PAGE * PAGE_BYTES + 16, 4),
        }
        let mut source = Cursor::new(bytes);
        let mut allowance = budget();
        let error = MultivalueReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .err()
        .unwrap();
        match variant {
            0 => assert_eq!(
                error,
                InspectError::ComplexReference(ComplexReferenceViolation::MissingParent)
            ),
            1 => assert_eq!(
                error,
                InspectError::ComplexReference(ComplexReferenceViolation::DuplicateChild)
            ),
            2 => assert_eq!(
                error,
                InspectError::ComplexReference(ComplexReferenceViolation::NullChild)
            ),
            _ => assert_eq!(error, InspectError::Malformed),
        }
    }
    let mut bytes = links_fixture::populated();
    rows(&mut bytes, &(0..101).map(Some).collect::<Vec<_>>());
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    assert!(matches!(
        MultivalueReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false
        ),
        Err(InspectError::StructureLimit)
    ));
}

#[test]
fn fresh_reads_reject_changed_unselected_parent_and_child_keys_and_become_terminal() {
    for variant in 0..5 {
        let bytes = links_fixture::populated();
        let mut source = MutableSource::new(bytes);
        let image = source.image();
        let mut allowance = budget();
        let mut reader = MultivalueReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .unwrap();
        {
            let mut bytes = image.borrow_mut();
            let child = usize::from(
                u16::from_le_bytes(
                    bytes[CHILD_PAGE * PAGE_BYTES + 18..CHILD_PAGE * PAGE_BYTES + 20]
                        .try_into()
                        .unwrap(),
                ) & 0x1fff,
            );
            let parent = usize::from(
                u16::from_le_bytes(
                    bytes[12 * PAGE_BYTES + 18..12 * PAGE_BYTES + 20]
                        .try_into()
                        .unwrap(),
                ) & 0x1fff,
            );
            match variant {
                0 => put32(&mut bytes, CHILD_PAGE * PAGE_BYTES + child + 2, 10),
                1 => put32(&mut bytes, CHILD_PAGE * PAGE_BYTES + child + 10, 9),
                2 => put32(&mut bytes, 12 * PAGE_BYTES + parent + 6, 40),
                3 => put32(&mut bytes, FLAT_PAGE * PAGE_BYTES + 16, 4),
                _ => put16(
                    &mut bytes,
                    FLAT_PAGE * PAGE_BYTES + FIELD_OFFSET + FIELD_BYTES + 23,
                    8,
                ),
            }
        }
        assert!(reader.read_parent(0).is_err());
        assert!(reader.is_failed());
        assert!(matches!(
            reader.read_parent(0),
            Err(InspectError::CollectionReaderFailed)
        ));
    }
}

#[test]
fn original_public_budget_and_cancellation_cover_open_and_complete_parent_publication() {
    let bytes = links_fixture::populated();
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut source = Cursor::new(&bytes);
    let mut measured = budget();
    let before = [
        measured.remaining(Resource::Bytes),
        measured.remaining(Resource::Rows),
        measured.remaining(Resource::Steps),
    ];
    {
        let mut reader = MultivalueReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            TableSnapshotLimits::default(),
            &mut measured,
            &observe,
        )
        .unwrap();
        assert_eq!(
            reader.read_parent(0).unwrap().values(),
            [Value::Int32(100), Value::Int32(101)]
        );
    }
    let used = [
        before[0] - measured.remaining(Resource::Bytes),
        before[1] - measured.remaining(Resource::Rows),
        before[2] - measured.remaining(Resource::Steps),
    ];
    for reduction in [None, Some(0), Some(1), Some(2)] {
        let mut remaining = used;
        if let Some(pool) = reduction {
            assert!(remaining[pool] > 0);
            remaining[pool] -= 1;
        }
        let mut controls = ResourceBudget::new(remaining[0], remaining[1], remaining[2]);
        let mut source = Cursor::new(&bytes);
        let result = (|| {
            let mut reader = MultivalueReader::open(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Score",
                TableSnapshotLimits::default(),
                &mut controls,
                &|| false,
            )?;
            reader.read_parent(0)
        })();
        if let Some(pool) = reduction {
            let expected = [Resource::Bytes, Resource::Rows, Resource::Steps][pool];
            assert!(matches!(result,Err(InspectError::Limit(error)) if error.resource==expected));
        } else {
            assert_eq!(
                result.unwrap().values(),
                [Value::Int32(100), Value::Int32(101)]
            );
            for pool in [Resource::Bytes, Resource::Rows, Resource::Steps] {
                assert_eq!(controls.remaining(pool), 0);
            }
        }
    }
    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut source = Cursor::new(&bytes);
        let mut controls = budget();
        let result = (|| {
            let mut reader = MultivalueReader::open(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Score",
                TableSnapshotLimits::default(),
                &mut controls,
                &cancel,
            )?;
            reader.read_parent(0)
        })();
        assert!(
            matches!(result, Err(InspectError::Cancelled)),
            "checkpoint {stop}"
        );
    }
}

#[test]
fn unselected_invalid_text_payload_is_not_decoded_or_certified_as_a_complete_column() {
    let mut bytes = links_fixture::populated();
    for (page, position) in [(TEMPLATE_PAGE, 0), (FLAT_PAGE, 1)] {
        let at = page * PAGE_BYTES + FIELD_OFFSET + position * FIELD_BYTES;
        bytes[at] = 10;
        bytes[at + 15] = 2;
        put16(&mut bytes, at + 7, 0);
        put16(&mut bytes, at + 23, 510);
        put16(&mut bytes, page * PAGE_BYTES + 43, 1);
    }
    put16(
        &mut bytes,
        FLAT_PAGE * PAGE_BYTES + FIELD_OFFSET + 2 * FIELD_BYTES + 21,
        4,
    );
    let records = [(10_i32, 1_i32, 0x61_u16), (10, 2, 0x62), (20, 3, 0xd800)]
        .into_iter()
        .map(|(key, id, unit)| {
            let mut row = 3u16.to_le_bytes().to_vec();
            row.extend(key.to_le_bytes());
            row.extend(id.to_le_bytes());
            row.extend(unit.to_le_bytes());
            row.extend(12u16.to_le_bytes());
            row.extend(10u16.to_le_bytes());
            row.extend(1u16.to_le_bytes());
            row.push(7);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[CHILD_PAGE * PAGE_BYTES..(CHILD_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(FLAT_PAGE as u32, &records));
    assert!(read(&bytes).is_err());
    let mut source = Cursor::new(&bytes);
    let mut allowance = budget();
    let mut reader = MultivalueReader::open(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Score",
        TableSnapshotLimits::default(),
        &mut allowance,
        &|| false,
    )
    .unwrap();
    assert_eq!(reader.value_column().data_type(), ScalarType::Text);
    assert!(reader.read_parent(2).unwrap().values().is_empty());
    assert_eq!(
        reader.read_parent(0).unwrap().values(),
        [Value::Text("a".into()), Value::Text("b".into())]
    );
    assert!(reader.read_parent(1).is_err());
    assert!(reader.is_failed());
    assert!(matches!(
        reader.read_parent(0),
        Err(InspectError::CollectionReaderFailed)
    ));
}

#[test]
fn null_parent_keys_and_empty_unsupported_declarations_keep_their_distinct_contracts() {
    let mut bytes = links_fixture::populated();
    let word = 12 * PAGE_BYTES + 18;
    let start = usize::from(u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap()) & 0x1fff);
    bytes[12 * PAGE_BYTES + start + 10] = 1;
    let mut source = Cursor::new(&bytes);
    let mut allowance = budget();
    let mut reader = MultivalueReader::open(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Score",
        TableSnapshotLimits::default(),
        &mut allowance,
        &|| false,
    )
    .unwrap();
    let empty = reader.read_parent(2).unwrap();
    assert_eq!(empty.key(), None);
    assert!(empty.values().is_empty());
    drop(reader);

    for extended in [false, true] {
        let mut bytes = links_fixture::populated();
        rows(&mut bytes, &[]);
        let at = TEMPLATE_PAGE * PAGE_BYTES + FIELD_OFFSET;
        if extended {
            bytes[at + 16] = 8;
        } else {
            bytes[at] = 0xff;
        }
        let mut source = Cursor::new(&bytes);
        let mut allowance = budget();
        assert!(
            MultivalueReader::open(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Score",
                TableSnapshotLimits::default(),
                &mut allowance,
                &|| false
            )
            .is_err()
        );
    }
    let mut source = Cursor::new(links_fixture::populated());
    let mut allowance = budget();
    assert!(matches!(
        MultivalueReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Id",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
}

#[test]
fn logical_child_roots_survive_forwarding_and_deleted_slot_gaps() {
    let mut bytes = links_fixture::populated();
    bytes.resize(25 * PAGE_BYTES, 0);
    let row = |key: i32, value: i32, identity: i32| {
        let mut record = 3u16.to_le_bytes().to_vec();
        record.extend(key.to_le_bytes());
        record.extend(value.to_le_bytes());
        record.extend(identity.to_le_bytes());
        record.push(7);
        record
    };
    let roots = [
        (0, row(10, 100, 1)),
        (0x8000, row(99, 999, 99)),
        (0x4000, (23u32 << 8).to_le_bytes().to_vec()),
        (0, row(20, 200, 3)),
    ];
    bytes[CHILD_PAGE * PAGE_BYTES..(CHILD_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(FLAT_PAGE as u32, &roots));
    bytes[23 * PAGE_BYTES..24 * PAGE_BYTES]
        .copy_from_slice(&data_page(FLAT_PAGE as u32, &[(0x8000, row(10, 101, 2))]));
    let parent_row = |identity: i32, key: i32| {
        let mut record = 2u16.to_le_bytes().to_vec();
        record.extend(identity.to_le_bytes());
        record.extend(key.to_le_bytes());
        record.push(3);
        record
    };
    let parent_roots = [
        (0, parent_row(1, 10)),
        (0x8000, parent_row(99, 99)),
        (0x4000, (24u32 << 8).to_le_bytes().to_vec()),
        (0, parent_row(3, 30)),
    ];
    bytes[12 * PAGE_BYTES..13 * PAGE_BYTES].copy_from_slice(&data_page(10, &parent_roots));
    bytes[24 * PAGE_BYTES..25 * PAGE_BYTES]
        .copy_from_slice(&data_page(10, &[(0x8000, parent_row(2, 20))]));

    assert_eq!(
        read(&bytes)
            .unwrap()
            .values(0)
            .unwrap()
            .cloned()
            .collect::<Vec<_>>(),
        [Value::Int32(100), Value::Int32(101)]
    );
    let mut source = Cursor::new(&bytes);
    let mut allowance = budget();
    let mut reader = MultivalueReader::open(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Score",
        TableSnapshotLimits::default(),
        &mut allowance,
        &|| false,
    )
    .unwrap();
    assert_eq!(
        reader.read_parent(0).unwrap().values(),
        [Value::Int32(100), Value::Int32(101)]
    );
    let forwarded_parent = reader.read_parent(1).unwrap();
    assert_eq!(forwarded_parent.values(), [Value::Int32(200)]);
    assert_eq!(forwarded_parent.row_reference().page_number(), 12);
    assert_eq!(forwarded_parent.row_reference().slot_number(), 2);
    assert_eq!(
        reader.read_parent(2).unwrap().row_reference().slot_number(),
        3
    );
}
