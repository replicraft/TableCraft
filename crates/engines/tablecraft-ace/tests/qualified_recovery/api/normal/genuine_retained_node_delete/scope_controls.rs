//! Readable original trees still require qualified write framing and topology.
use super::{budget, cases, fixtures, image, requests, scope};
use std::io::Cursor;
use tablecraft_ace::{EditError, RecoverableEditError, RowDelete, WriteBlocker, read_table_index};

#[test]
#[ignore = "requires four independent native/SDK-qualified retained NODE source images"]
fn emptying_original_children_matches_separately_qualified_retirement_sources() {
    use sha2::{Digest, Sha256};
    use tablecraft_testkit::access::genuine_node_retirement as retirement;

    let expected = retirement::load_all(|raw, wanted| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, wanted);
    });
    let mut accepted = 0;
    for case in cases() {
        for (removed, recipe) in [
            (302, retirement::Recipe::Left),
            (800, retirement::Recipe::All),
        ] {
            let qualified = expected
                .iter()
                .find(|source| source.before == case.before && source.recipe == recipe)
                .unwrap();
            let requests = (0..removed)
                .map(|ordinal| RowDelete::new(ordinal, fixtures::record(ordinal)))
                .collect();
            let plan = image(&case.before)
                .plan_recoverable_ordinary_row_deletes("Items", requests, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), qualified.after);
            assert_eq!(plan.deleted_rows(), removed);
            accepted += 1;
        }
    }
    assert_eq!(accepted, 4);
}

#[test]
#[ignore = "requires four independent native/SDK-qualified retained NODE source images"]
fn readable_reserved_stamps_refuse_node_and_child_edits_including_empty_requests() {
    let mut refused = 0;
    for case in cases() {
        for page in [case.roots[0], case.left[0]] {
            let mut unknown = case.before.clone();
            assert_eq!(&unknown[page * 4096 + 8..page * 4096 + 12], &[0; 4]);
            unknown[page * 4096 + 8] = 1;
            let read = read_table_index(
                &mut Cursor::new(&unknown),
                scope(),
                "Items",
                "PrimaryID",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(read.rows(), fixtures::rows(false, false));
            for empty in [false, true] {
                let requested = if empty { vec![] } else { requests(false) };
                let error = image(&unknown)
                    .plan_recoverable_ordinary_row_deletes(
                        "Items",
                        requested,
                        &mut budget(),
                        &|| false,
                    )
                    .err()
                    .expect("readability does not qualify unknown native write framing");
                assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::IndexedColumn))
                ));
                refused += 1;
            }
        }
    }
    assert_eq!(refused, 8);
}
