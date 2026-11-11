//! Saved-query composition reuses the same independent native fragment writer.
use super::{EMPTY_ID, Fragment, PAGE_BYTES, QUERY_ID, install_query_table};
use crate::fixtures::access::{attachment_modern, data_page, put32, record};

/// Append ordinary Saved/Empty query declarations without changing attachment storage.
/// Fragments remain original native fields; no SQL interpolation or producer codec runs.
pub fn database_with(fragments: &[Fragment<'_>]) -> Vec<u8> {
    let mut bytes = attachment_modern::database(6);
    let definition = bytes.len() / PAGE_BYTES;
    let mut rows = catalog_rows(&bytes);
    let original_count = rows.len();
    for (name, id, kind, flags) in [
        (
            "MSysQueries",
            i32::try_from(definition).unwrap(),
            1,
            0x8000_0000,
        ),
        ("Saved", QUERY_ID, 5, 0),
        ("Empty", EMPTY_ID, 5, 0),
    ] {
        rows.push((
            0,
            record(
                id,
                0,
                kind,
                flags,
                &[b"\xff\xfe".as_slice(), name.as_bytes()].concat(),
            ),
        ));
    }
    assert_eq!(
        original_count, 6,
        "the actual attachment catalog contains six roots"
    );
    put32(
        &mut bytes,
        2 * PAGE_BYTES + 16,
        u32::try_from(rows.len()).unwrap(),
    );
    bytes[4 * PAGE_BYTES..5 * PAGE_BYTES].copy_from_slice(&data_page(2, &rows));
    install_query_table(
        &mut bytes,
        fragments,
        definition,
        definition + 1,
        definition + 2,
    );
    bytes
}

fn catalog_rows(bytes: &[u8]) -> Vec<(u16, Vec<u8>)> {
    let page = &bytes[4 * PAGE_BYTES..5 * PAGE_BYTES];
    let count = usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap()));
    let mut end = PAGE_BYTES;
    let mut rows = Vec::new();
    for slot in 0..count {
        let at = 14 + 2 * slot;
        let word = u16::from_le_bytes(page[at..at + 2].try_into().unwrap());
        assert_eq!(
            word & 0xe000,
            0,
            "the raw attachment catalog has direct live roots"
        );
        let start = usize::from(word & 0x1fff);
        assert!(start <= end);
        rows.push((0, page[start..end].to_vec()));
        end = start;
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_keeps_all_existing_attachment_storage_and_appends_its_query_owner() {
        let before = attachment_modern::database(6);
        let after = database_with(&[
            Fragment {
                object: Some(QUERY_ID),
                attribute: Some(5),
                name1: Some("Items"),
                ..Default::default()
            },
            Fragment {
                object: Some(QUERY_ID),
                attribute: Some(6),
                expression: Some("*"),
                ..Default::default()
            },
        ]);
        assert_eq!(after.len(), before.len() + 3 * PAGE_BYTES);
        for page in 0..before.len() / PAGE_BYTES {
            if !matches!(page, 2 | 4) {
                assert_eq!(
                    &after[page * PAGE_BYTES..(page + 1) * PAGE_BYTES],
                    &before[page * PAGE_BYTES..(page + 1) * PAGE_BYTES],
                    "original page {page}"
                );
            }
        }
        let rows = catalog_rows(&after);
        let original = catalog_rows(&before);
        assert_eq!(&rows[..original.len()], original);
        let definition = before.len() / PAGE_BYTES;
        assert_eq!(
            &after[definition * PAGE_BYTES..definition * PAGE_BYTES + 2],
            [2, 1]
        );
        assert_eq!(
            u32::from_le_bytes(
                after[definition * PAGE_BYTES + 55..definition * PAGE_BYTES + 59]
                    .try_into()
                    .unwrap()
            ),
            ((definition + 1) as u32) << 8
        );
    }
}
