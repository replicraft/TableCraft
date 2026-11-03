//! Complete history keys remain separate from current Memo and selected payloads.
use super::*;
use tablecraft_ace::{ParentVersionHistory, VersionHistoryReader};
use tablecraft_testkit::support::ace::{
    mutable_source::MutableSource, page_reads::PageReads, resources::assert_public_boundary,
};

fn ordered_history() -> Vec<u8> {
    fixture::database_with(&[
        (10, 9, Some("é🦀\0stored"), Some(1)),
        (10, -7, Some(""), Some(0.25f64.to_bits())),
        (10, 2, Some("older"), Some((-0.25f64).to_bits())),
        (20, i32::MIN, None, None),
    ])
}

fn assert_stored(parent: &ParentVersionHistory) {
    assert_eq!(parent.key(), Some(10));
    let rows = parent
        .stored_versions()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        rows.iter().map(|row| row.identity()).collect::<Vec<_>>(),
        [9, -7, 2]
    );
    assert_eq!(
        rows.iter().map(|row| row.value()).collect::<Vec<_>>(),
        [Some("é🦀\0stored"), Some(""), Some("older")]
    );
    assert_eq!(
        rows.iter()
            .map(|row| row.modified().unwrap().to_bits())
            .collect::<Vec<_>>(),
        [1, 0.25f64.to_bits(), (-0.25f64).to_bits()]
    );
}

#[test]
fn stored_order_nulls_and_exact_date_bits_survive_reader_drop_without_latest_selection() {
    for code in [5, 6] {
        let mut bytes = ordered_history();
        bytes[20] = code;
        let first;
        {
            let mut source = Cursor::new(&bytes);
            let mut allowance = budget();
            let mut reader = VersionHistoryReader::open(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "nOtEs",
                TableSnapshotLimits::default(),
                &mut allowance,
                &|| false,
            )
            .unwrap();
            assert_eq!(reader.memo_column().name(), "Notes");
            assert_eq!(
                reader
                    .parents()
                    .iter()
                    .map(|parent| (parent.key(), parent.child_count()))
                    .collect::<Vec<_>>(),
                [(Some(10), 3), (Some(20), 1), (Some(30), 0)]
            );
            assert!(matches!(
                reader.read_parent(3),
                Err(InspectError::InvalidPageRange)
            ));
            assert!(!reader.is_failed());
            first = reader.read_parent(0).unwrap();
            assert_stored(&first);
            let second = reader.read_parent(1).unwrap();
            let null = second.stored_versions().next().unwrap().unwrap();
            assert_eq!(null.identity(), i32::MIN);
            assert_eq!(null.value(), None);
            assert_eq!(null.modified(), None);
            let empty = reader.read_parent(2).unwrap();
            assert_eq!(empty.key(), Some(30));
            assert_eq!(empty.stored_versions().len(), 0);
        }
        assert_stored(&first);
    }
}

fn external_descriptor(page: u32) -> Vec<u8> {
    let mut bytes = 0x40000002u32.to_le_bytes().to_vec();
    bytes.extend((page << 8).to_le_bytes());
    bytes.extend([0; 4]);
    bytes
}

#[test]
fn current_visible_memo_payload_is_independent_of_stored_history() {
    let mut bytes = fixture::with_current_memo_descriptor(&external_descriptor(23));
    bytes.resize(24 * PAGE_BYTES, 0);
    let mut source = PageReads::new(bytes.clone());
    {
        let mut allowance = budget();
        let mut reader = VersionHistoryReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Notes",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .unwrap();
        let first = reader.read_parent(0).unwrap();
        assert_eq!(
            first
                .stored_versions()
                .map(|row| row.unwrap().value().map(str::to_owned))
                .collect::<Vec<_>>(),
            [Some("é🦀\0older".to_owned()), Some("newer".to_owned())]
        );
    }
    assert_eq!(source.count(23), 0);
    // The separate explicit current-Memo scalar consumer still observes its
    // malformed external payload. History never substitutes a stored value.
    assert!(
        tablecraft_ace::read_table_projection_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &["Notes"],
            TablePageRequest::new(0, 1).unwrap(),
            &mut budget(),
            &|| false,
        )
        .is_err()
    );
}

fn row(bytes: &[u8], page: usize, slot: usize) -> Vec<u8> {
    let word = page * PAGE_BYTES + 14 + 2 * slot;
    let start = usize::from(u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap()) & 0x1fff);
    let end = if slot == 0 {
        PAGE_BYTES
    } else {
        usize::from(u16::from_le_bytes(bytes[word - 2..word].try_into().unwrap()) & 0x1fff)
    };
    bytes[page * PAGE_BYTES + start..page * PAGE_BYTES + end].to_vec()
}

fn bad_history_payload() -> Vec<u8> {
    let mut bytes = database();
    let mut first = 4u16.to_le_bytes().to_vec();
    first.extend(10i32.to_le_bytes());
    first.extend((-0.25f64).to_bits().to_le_bytes());
    first.extend(2i32.to_le_bytes());
    first.extend(external_descriptor(23));
    first.extend(30u16.to_le_bytes());
    first.extend(18u16.to_le_bytes());
    first.extend(1u16.to_le_bytes());
    first.push(15);
    let rows = [
        (0, first),
        (0, row(&bytes, 22, 1)),
        (0, row(&bytes, 22, 2)),
        (0, row(&bytes, 22, 3)),
    ];
    bytes[22 * PAGE_BYTES..23 * PAGE_BYTES].copy_from_slice(&support::data_page(20, &rows));
    bytes.resize(24 * PAGE_BYTES, 0);
    bytes
}

#[test]
fn unselected_historical_text_is_unread_and_selected_storage_errors_are_terminal() {
    let bytes = bad_history_payload();
    assert!(read(&bytes, "Notes").is_err());
    let mut source = PageReads::new(bytes);
    {
        let mut allowance = budget();
        let mut reader = VersionHistoryReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Notes",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .unwrap();
        let second = reader.read_parent(1).unwrap();
        let versions = second
            .stored_versions()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(versions[0].value(), Some(""));
        assert_eq!(versions[0].modified().unwrap().to_bits(), 1);
        assert_eq!(versions[1].value(), None);
    }
    assert_eq!(source.count(23), 0);
    {
        let mut allowance = budget();
        let mut reader = VersionHistoryReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Notes",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .unwrap();
        assert!(reader.read_parent(0).is_err());
        assert!(reader.is_failed());
        assert!(matches!(
            reader.read_parent(1),
            Err(InspectError::CollectionReaderFailed)
        ));
    }
    assert!(source.count(23) > 0);
}

#[test]
fn nullable_hidden_keys_and_declaration_rules_do_not_depend_on_requested_parent_size() {
    let mut bytes = database();
    let word = 12 * PAGE_BYTES + 18;
    let start = usize::from(u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap()) & 0x1fff);
    bytes[12 * PAGE_BYTES + start + 16] = 1;
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    let mut reader = VersionHistoryReader::open(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Notes",
        TableSnapshotLimits::default(),
        &mut allowance,
        &|| false,
    )
    .unwrap();
    let empty = reader.read_parent(2).unwrap();
    assert_eq!(empty.key(), None);
    assert_eq!(empty.stored_versions().len(), 0);
    drop(reader);

    for flags in [0x02, 0x82] {
        for extended in [0, 1] {
            let mut bytes = database();
            bytes[10 * PAGE_BYTES + 63 + 2 * 25 + 15] = flags;
            bytes[10 * PAGE_BYTES + 63 + 2 * 25 + 16] = extended;
            let mut source = Cursor::new(bytes);
            let mut allowance = budget();
            assert!(
                VersionHistoryReader::open(
                    &mut source,
                    CatalogScope::new(1),
                    "Items",
                    "Notes",
                    TableSnapshotLimits::default(),
                    &mut allowance,
                    &|| false,
                )
                .is_ok()
            );
        }
    }
    for memo in ["Score", "Id", "missing"] {
        let mut source = Cursor::new(database());
        let mut allowance = budget();
        let error = VersionHistoryReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            memo,
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .err()
        .unwrap();
        assert_eq!(
            error,
            if memo == "missing" {
                InspectError::UnknownColumn
            } else {
                InspectError::Unsupported
            }
        );
    }
    for (page, field, flags) in [(10, 2, 0x80), (20, 1, 0x10)] {
        let mut bytes = database();
        let offset = if page == 10 { 15 } else { 16 };
        bytes[page * PAGE_BYTES + 63 + field * 25 + offset] = flags;
        let mut source = Cursor::new(bytes);
        let mut allowance = budget();
        assert!(matches!(
            VersionHistoryReader::open(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Notes",
                TableSnapshotLimits::default(),
                &mut allowance,
                &|| false,
            ),
            Err(InspectError::Unsupported)
        ));
    }
}

#[test]
fn forwarded_roots_and_deleted_gaps_preserve_logical_history_membership() {
    let mut bytes = database();
    let children = [
        row(&bytes, 22, 0),
        row(&bytes, 22, 1),
        row(&bytes, 22, 2),
        row(&bytes, 22, 3),
    ];
    let parents = [row(&bytes, 12, 0), row(&bytes, 12, 1), row(&bytes, 12, 2)];
    bytes.resize(25 * PAGE_BYTES, 0);
    let roots = [
        (0, children[0].clone()),
        (0x8000, children[0].clone()),
        (0x4000, (23u32 << 8).to_le_bytes().to_vec()),
        (0, children[2].clone()),
        (0, children[3].clone()),
    ];
    bytes[22 * PAGE_BYTES..23 * PAGE_BYTES].copy_from_slice(&support::data_page(20, &roots));
    bytes[23 * PAGE_BYTES..24 * PAGE_BYTES]
        .copy_from_slice(&support::data_page(20, &[(0x8000, children[1].clone())]));
    let roots = [
        (0, parents[0].clone()),
        (0x8000, parents[0].clone()),
        (0x4000, (24u32 << 8).to_le_bytes().to_vec()),
        (0, parents[2].clone()),
    ];
    bytes[12 * PAGE_BYTES..13 * PAGE_BYTES].copy_from_slice(&support::data_page(10, &roots));
    bytes[24 * PAGE_BYTES..25 * PAGE_BYTES]
        .copy_from_slice(&support::data_page(10, &[(0x8000, parents[1].clone())]));
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    let mut reader = VersionHistoryReader::open(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Notes",
        TableSnapshotLimits::default(),
        &mut allowance,
        &|| false,
    )
    .unwrap();
    let first = reader.read_parent(0).unwrap();
    assert_eq!(
        first
            .stored_versions()
            .map(|version| version.unwrap().identity())
            .collect::<Vec<_>>(),
        [2, 9]
    );
    let second = reader.read_parent(1).unwrap();
    assert_eq!(second.row_reference().page_number(), 12);
    assert_eq!(second.row_reference().slot_number(), 2);
    assert_eq!(
        second
            .stored_versions()
            .map(|version| version.unwrap().identity())
            .collect::<Vec<_>>(),
        [7, 3]
    );
    assert_eq!(
        reader.read_parent(2).unwrap().row_reference().slot_number(),
        3
    );
}

#[test]
fn changed_unselected_keys_or_visible_memo_metadata_invalidate_the_directory() {
    for child_change in [true, false] {
        let mut source = MutableSource::new(database());
        let image = source.image();
        let mut allowance = budget();
        let mut reader = VersionHistoryReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Notes",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        )
        .unwrap();
        {
            let mut bytes = image.borrow_mut();
            if child_change {
                let word = 22 * PAGE_BYTES + 14;
                let start = usize::from(
                    u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap()) & 0x1fff,
                );
                bytes[22 * PAGE_BYTES + start + 2..22 * PAGE_BYTES + start + 6]
                    .copy_from_slice(&20i32.to_le_bytes());
            } else {
                // Both visible declaration families qualify independently, but
                // this source-bound directory must reject changed metadata.
                bytes[10 * PAGE_BYTES + 63 + 2 * 25 + 15] = 0x82;
            }
        }
        assert!(reader.read_parent(2).is_err());
        assert!(reader.is_failed());
        assert!(matches!(
            reader.read_parent(0),
            Err(InspectError::CollectionReaderFailed)
        ));
    }
}

#[test]
fn full_public_history_reader_is_measured_at_exact_and_one_less_resource_limits() {
    let bytes = ordered_history();
    assert_public_boundary(
        budget,
        |allowance, cancelled| {
            let mut source = Cursor::new(&bytes);
            let mut reader = VersionHistoryReader::open(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Notes",
                TableSnapshotLimits::default(),
                allowance,
                cancelled,
            )?;
            reader.read_parent(0)
        },
        assert_stored,
        assert_stored,
    );
}

#[test]
fn cancellation_after_open_keeps_history_reader_terminal_without_a_partial_list() {
    let stopped = Cell::new(false);
    let cancelled = || stopped.get();
    let mut source = Cursor::new(database());
    let mut allowance = budget();
    let mut reader = VersionHistoryReader::open(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Notes",
        TableSnapshotLimits::default(),
        &mut allowance,
        &cancelled,
    )
    .unwrap();
    stopped.set(true);
    assert!(matches!(
        reader.read_parent(0),
        Err(InspectError::Cancelled)
    ));
    assert!(reader.is_failed());
    stopped.set(false);
    assert!(matches!(
        reader.read_parent(1),
        Err(InspectError::CollectionReaderFailed)
    ));
}

#[test]
fn a_changed_unrelated_template_cannot_create_an_ambiguous_visible_memo_binding() {
    let original = fixture::mixed::database();
    let mut source = MutableSource::new(original.clone());
    let image = source.image();
    let mut allowance = budget();
    let mut reader = VersionHistoryReader::open(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Notes",
        TableSnapshotLimits::default(),
        &mut allowance,
        &|| false,
    )
    .unwrap();
    {
        let mut bytes = image.borrow_mut();
        let template =
            fixture::definition(&[("Notes", 12, 0, 0), ("Modified", 8, 0, 8)], 0, 25 << 8);
        bytes[24 * PAGE_BYTES..25 * PAGE_BYTES].copy_from_slice(&template);
        let mut flat = fixture::definition(
            &[
                ("Owner", 4, 0, 4),
                ("Notes", 12, 0, 0),
                ("Modified", 8, 4, 8),
                ("ChildID", 4, 12, 4),
            ],
            0,
            28 << 8,
        );
        flat[63 + 16] = 8;
        flat[63 + 3 * 25 + 15] = 7;
        bytes[27 * PAGE_BYTES..28 * PAGE_BYTES].copy_from_slice(&flat);
        // The original owner's definitions, selected history and all of its
        // membership/payload bytes are unchanged. Only another template changed.
        for page in [10, 12, 17, 20, 22] {
            assert_eq!(
                &bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES],
                &original[page * PAGE_BYTES..(page + 1) * PAGE_BYTES],
            );
        }
    }
    let changed = image.borrow().clone();
    let complete = tablecraft_ace::inspect_complex_schema(
        &mut Cursor::new(&changed),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(complete.columns().len(), 2);
    assert_eq!(read(&changed, "Notes").err(), Some(InspectError::Malformed));
    // Empty membership does not bypass the fresh all-template association check.
    assert!(matches!(
        reader.read_parent(2),
        Err(InspectError::Malformed)
    ));
    assert!(reader.is_failed());
    assert!(matches!(
        reader.read_parent(0),
        Err(InspectError::CollectionReaderFailed)
    ));
}
