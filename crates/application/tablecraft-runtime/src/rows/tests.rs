use super::*;
use crate::CellKind;
use tablecraft_ace::InspectError;
use tablecraft_testkit::fixtures::access::{calculated_storage, mutation_fixture};

const FIXTURE_PAGE_BYTES: usize = 4096;
const FIXTURE_DATA_PAGE: usize = 12;
const FIXTURE_SLOT_DIRECTORY: usize = 14;

fn opened(bytes: &[u8]) -> (tempfile::TempDir, Document) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pages.mdb");
    std::fs::write(&path, bytes).unwrap();
    let (document, _) = Document::open(&path, &mut String::new(), 1, &|| false).unwrap();
    (directory, document)
}

#[test]
fn scalar_and_stored_pages_publish_only_display_rows_and_check_exact_eof() {
    for count in [100, 101, 200] {
        let records = (1..=count).map(|id| (id, id * 10)).collect::<Vec<_>>();
        let cached = vec![None; count as usize];
        let cases = [
            (false, mutation_fixture::with_rows(&records)),
            (true, calculated_storage::database(&cached)),
        ];
        for (stored_calculation, original) in cases {
            let (directory, mut document) = opened(&original);

            for offset in (0..count).step_by(100) {
                let Response::Data {
                    offset: actual_offset,
                    rows,
                    has_more,
                    ..
                } = document.page("Items", offset as u32, &|| false).unwrap()
                else {
                    panic!("expected native page");
                };
                let displayed = (count - offset).min(100);
                assert_eq!(actual_offset, offset as u32);
                assert_eq!(rows.len(), displayed as usize);
                assert_eq!(rows[0][0].text, (offset + 1).to_string());
                assert_eq!(
                    rows.last().unwrap()[0].text,
                    (offset + displayed).to_string()
                );
                assert_eq!(has_more, offset + displayed < count);
                for row in &rows {
                    let CellKind::Scalar {
                        is_null,
                        stored_calculation: actual_stored,
                        ..
                    } = row[1].kind
                    else {
                        panic!("expected scalar or stored calculated value");
                    };
                    assert_eq!(actual_stored, stored_calculation);
                    assert_eq!(is_null, stored_calculation);
                }
            }

            assert_eq!(
                std::fs::read(directory.path().join("pages.mdb")).unwrap(),
                original,
                "paging preserves the source"
            );
        }
    }
}

#[test]
fn invalid_lookahead_refuses_the_whole_page_instead_of_publishing_a_prefix() {
    let records = (1..=101).map(|id| (id, id * 10)).collect::<Vec<_>>();
    let mut bytes = mutation_fixture::with_rows(&records);
    // Independently corrupt only the 101st literal fixture record. Its slot is
    // valid, but the impossible stored column count cannot be decoded.
    let data_start = FIXTURE_DATA_PAGE * FIXTURE_PAGE_BYTES;
    let slot = data_start + FIXTURE_SLOT_DIRECTORY + 100 * size_of::<u16>();
    let start = u16::from_le_bytes(bytes[slot..slot + 2].try_into().unwrap()) as usize;
    bytes[data_start + start..data_start + start + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    let (_directory, mut document) = opened(&bytes);

    // Establish that the visible 100-row prefix itself is fully readable.
    let mut browser = NativeBrowser::new(&mut document.source, SCOPE, budget(), &|| false);
    let page = browser
        .browse_page(
            "Items",
            CollectionPageRequest::new(
                TablePageRequest::new(0, 100).unwrap(),
                TableSnapshotLimits::default(),
            ),
        )
        .unwrap();
    let BrowsePage::Scalar(page) = page else {
        panic!("expected scalar prefix");
    };
    assert_eq!(page.rows().len(), 100);

    assert!(matches!(
        document.page("Items", 0, &|| false),
        Err(Error::Native(InspectError::Unsupported))
    ));
}
