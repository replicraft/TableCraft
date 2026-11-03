//! The real browser lends its source and original controls to the typed reader.
use super::*;
use tablecraft_ace::{AttachmentReader, TableSnapshotLimits};
use tablecraft_testkit::{
    fixtures::access::{attachment_modern as modern, attachments as raw},
    support::ace::{mutable_source::MutableSource, page_reads::PageReads},
};

const PAGE: usize = 4096;

#[test]
fn browser_attachment_results_own_encoded_values_under_original_cumulative_controls() {
    let bytes = modern::database(6);
    let mut measured = ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000);
    let mut source = Cursor::new(&bytes);
    {
        let mut reader = AttachmentReader::open(
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
            .attachment_reader("Items", "Score", TableSnapshotLimits::default())
            .unwrap();
        result = reader.read_parent(0).unwrap();
        assert_eq!(result.attachments().len(), 2);
        assert!(matches!(reader.read_parent(1), Err(InspectError::Limit(_))));
        assert!(reader.is_failed());
        assert!(matches!(
            reader.read_parent(0),
            Err(InspectError::CollectionReaderFailed)
        ));
    }
    let first = result.attachments().next().unwrap().unwrap();
    assert_eq!(first.file_name(), Some("synthetic.bin"));
    assert_eq!(first.encoded_file_data(), Some(raw::PAYLOAD));
    assert_eq!(
        result
            .attachments()
            .nth(1)
            .unwrap()
            .unwrap()
            .encoded_file_data(),
        None
    );
}

#[test]
fn browser_reader_leaves_unselected_external_file_data_unread() {
    let mut descriptor = (0x40000000u32 | raw::PAYLOAD.len() as u32)
        .to_le_bytes()
        .to_vec();
    descriptor.extend((23u32 << 8).to_le_bytes());
    descriptor.extend([0; 4]);
    let mut bytes = modern::with_first_data(6, &descriptor);
    bytes.resize(24 * PAGE, 0);
    let mut source = PageReads::new(bytes);
    {
        let mut browser = NativeBrowser::new(
            &mut source,
            CatalogScope::new(1),
            ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000),
            &|| false,
        );
        let mut reader = browser
            .attachment_reader("Items", "Score", TableSnapshotLimits::default())
            .unwrap();
        assert_eq!(reader.parents()[0].child_count(), 2);
        assert_eq!(reader.read_parent(1).unwrap().attachments().len(), 1);
    }
    assert_eq!(source.count(23), 0);
}

#[test]
fn browser_reopen_rebuilds_changed_parent_keys_after_terminal_reader_failure() {
    let source = MutableSource::new(modern::database(6));
    let image = source.image();
    let mut browser = NativeBrowser::new(
        source,
        CatalogScope::new(1),
        ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000),
        &|| false,
    );
    {
        let mut reader = browser
            .attachment_reader("Items", "Score", TableSnapshotLimits::default())
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
        .attachment_reader("Items", "Score", TableSnapshotLimits::default())
        .unwrap();
    assert_eq!(reader.parents()[2].key(), Some(40));
    assert_eq!(reader.read_parent(2).unwrap().attachments().len(), 0);
    assert_eq!(reader.read_parent(0).unwrap().attachments().len(), 2);
}
