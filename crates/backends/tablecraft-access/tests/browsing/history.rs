//! The real browser lends source ownership and original controls to history reads.
use super::*;
use tablecraft_ace::{TableSnapshotLimits, VersionHistoryReader};
use tablecraft_testkit::{
    fixtures::access::version_history as raw,
    support::ace::{mutable_source::MutableSource, page_reads::PageReads},
};

const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000)
}

#[test]
fn browser_history_results_own_exact_stored_values_under_original_cumulative_controls() {
    let bytes = raw::database();
    let mut measured = budget();
    let mut source = Cursor::new(&bytes);
    {
        let mut reader = VersionHistoryReader::open(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Notes",
            TableSnapshotLimits::default(),
            &mut measured,
            &|| false,
        )
        .unwrap();
        reader.read_parent(0).unwrap();
    }
    let rows = 16384 - measured.remaining(tablecraft_core::limits::Resource::Rows);
    let result;
    {
        let mut browser = NativeBrowser::new(
            Cursor::new(&bytes),
            CatalogScope::new(1),
            ResourceBudget::new(64 * 1024 * 1024, rows, 1_000_000),
            &|| false,
        );
        let mut reader = browser
            .version_history_reader("Items", "Notes", TableSnapshotLimits::default())
            .unwrap();
        assert_eq!(reader.memo_column().name(), "Notes");
        result = reader.read_parent(0).unwrap();
        assert!(matches!(reader.read_parent(1), Err(InspectError::Limit(_))));
        assert!(reader.is_failed());
        assert!(matches!(
            reader.read_parent(0),
            Err(InspectError::CollectionReaderFailed)
        ));
    }
    let stored = result
        .stored_versions()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        stored
            .iter()
            .map(|version| version.identity())
            .collect::<Vec<_>>(),
        [2, 9]
    );
    assert_eq!(stored[0].value(), Some("é🦀\0older"));
    assert_eq!(
        stored[0].modified().unwrap().to_bits(),
        (-0.25f64).to_bits()
    );
    assert_eq!(stored[1].value(), Some("newer"));
    assert_eq!(stored[1].modified().unwrap().to_bits(), 0.25f64.to_bits());
}

#[test]
fn browser_history_does_not_fetch_current_visible_memo_external_storage() {
    let mut descriptor = 0x40000002u32.to_le_bytes().to_vec();
    descriptor.extend((23u32 << 8).to_le_bytes());
    descriptor.extend([0; 4]);
    let mut bytes = raw::with_current_memo_descriptor(&descriptor);
    bytes.resize(24 * PAGE, 0);
    let mut source = PageReads::new(bytes);
    {
        let mut browser =
            NativeBrowser::new(&mut source, CatalogScope::new(1), budget(), &|| false);
        let mut reader = browser
            .version_history_reader("Items", "Notes", TableSnapshotLimits::default())
            .unwrap();
        assert_eq!(reader.parents()[0].child_count(), 2);
        let stored = reader.read_parent(0).unwrap();
        assert_eq!(
            stored
                .stored_versions()
                .map(|version| version.unwrap().value().map(str::to_owned))
                .collect::<Vec<_>>(),
            [Some("é🦀\0older".to_owned()), Some("newer".to_owned())]
        );
    }
    assert_eq!(source.count(23), 0);
}

#[test]
fn browser_explicit_reopen_rebuilds_history_keys_after_terminal_source_change() {
    let source = MutableSource::new(raw::database());
    let image = source.image();
    let mut browser = NativeBrowser::new(source, CatalogScope::new(1), budget(), &|| false);
    {
        let mut reader = browser
            .version_history_reader("Items", "Notes", TableSnapshotLimits::default())
            .unwrap();
        {
            let mut bytes = image.borrow_mut();
            let word = 12 * PAGE + 18;
            let start =
                usize::from(u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap()) & 0x1fff);
            bytes[12 * PAGE + start + 6..12 * PAGE + start + 10]
                .copy_from_slice(&40i32.to_le_bytes());
        }
        assert!(reader.read_parent(0).is_err());
        assert!(reader.is_failed());
    }
    let mut reader = browser
        .version_history_reader("Items", "Notes", TableSnapshotLimits::default())
        .unwrap();
    assert_eq!(reader.parents()[2].key(), Some(40));
    assert_eq!(reader.read_parent(2).unwrap().stored_versions().len(), 0);
    assert_eq!(reader.read_parent(0).unwrap().stored_versions().len(), 2);
}
