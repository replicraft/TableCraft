//! NativeBrowser owns the same complete directory and original read controls.
use super::*;
use tablecraft_ace::MultivalueReader;
use tablecraft_testkit::support::ace::mutable_source::MutableSource;

#[test]
fn browser_reader_keeps_owned_results_and_spends_its_original_cumulative_budget() {
    let bytes = links_fixture::populated();
    let mut measured = ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000);
    let mut source = Cursor::new(&bytes);
    {
        let mut reader = MultivalueReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            TableSnapshotLimits::default(),
            &mut measured,
            &|| false,
        )
        .unwrap();
        reader.read_parent(0).unwrap();
    }
    let rows = 16384 - measured.remaining(Resource::Rows);
    let mut browser = NativeBrowser::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        ResourceBudget::new(64 * 1024 * 1024, rows, 1_000_000),
        &|| false,
    );
    let result;
    {
        let mut reader = browser
            .multivalue_reader("Items", "Score", TableSnapshotLimits::default())
            .unwrap();
        result = reader.read_parent(0).unwrap();
        assert_eq!(result.values(), [Value::Int32(100), Value::Int32(101)]);
        assert!(matches!(reader.read_parent(1), Err(InspectError::Limit(_))));
        assert!(reader.is_failed());
        assert!(matches!(
            reader.read_parent(0),
            Err(InspectError::CollectionReaderFailed)
        ));
    }
    assert_eq!(result.values(), [Value::Int32(100), Value::Int32(101)]);
}

#[test]
fn source_bound_reader_rechecks_unselected_parent_keys_and_reopen_rebuilds_membership() {
    let source = MutableSource::new(links_fixture::populated());
    let image = source.image();
    let mut browser = NativeBrowser::new(
        source,
        CatalogScope::new(1),
        ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000),
        &|| false,
    );
    {
        let mut reader = browser
            .multivalue_reader("Items", "Score", TableSnapshotLimits::default())
            .unwrap();
        {
            let mut bytes = image.borrow_mut();
            let start = usize::from(
                u16::from_le_bytes(bytes[12 * 4096 + 18..12 * 4096 + 20].try_into().unwrap())
                    & 0x1fff,
            );
            support::put32(&mut bytes, 12 * 4096 + start + 6, 40);
        }
        assert!(reader.read_parent(0).is_err());
        assert!(reader.is_failed());
    }
    let mut reader = browser
        .multivalue_reader("Items", "Score", TableSnapshotLimits::default())
        .unwrap();
    assert_eq!(reader.parents()[2].key(), Some(40));
    assert!(reader.read_parent(2).unwrap().values().is_empty());
    assert_eq!(
        reader.read_parent(0).unwrap().values(),
        [Value::Int32(100), Value::Int32(101)]
    );
}
