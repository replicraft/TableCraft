//! Independent tombstone overlays and original-request refusal controls.
use super::proof;
use crate::{EditError, NativeEditPlan, NativeImage, RowDelete};
use sha2::{Digest, Sha256};
use std::{io::Cursor, path::Path};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_ordinary_content::{self as model, Case};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn scope() -> CatalogScope {
    CatalogScope::new(147)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn expected(before: &[u8], definition: usize, data: usize) -> (Vec<u8>, Vec<u8>) {
    let base = data * 4096;
    assert_eq!(
        u16::from_le_bytes(before[base + 12..base + 14].try_into().unwrap()),
        1
    );
    let word = u16::from_le_bytes(before[base + 14..base + 16].try_into().unwrap());
    assert_eq!(word & 0xe000, 0);
    assert_eq!(
        &before[definition * 4096 + 16..definition * 4096 + 20],
        &[1, 0, 0, 0]
    );
    let record = before[base + usize::from(word)..base + 4096].to_vec();
    let mut after = before.to_vec();
    after[base + 14..base + 16].copy_from_slice(&(word | 0xc000).to_le_bytes());
    after[definition * 4096 + 16..definition * 4096 + 20].fill(0);
    (record, after)
}

fn exercise(before: &[u8], table: &str, definition: usize, data: usize) {
    let (raw, after) = expected(before, definition, data);
    let captured = image(before);
    let prepared = captured
        .prepare_ordinary_row_deletes(table, &[0], &mut budget(), &|| false)
        .unwrap();
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].row(), 0);
    assert_eq!(prepared[0].expected_bytes(), raw);
    assert!(
        captured
            .prepare_ordinary_row_deletes(table, &[], &mut budget(), &|| false)
            .unwrap()
            .is_empty()
    );
    assert!(
        captured
            .prepare_ordinary_row_deletes(table, &[0, 0], &mut budget(), &|| false)
            .is_err()
    );
    assert!(matches!(
        captured.prepare_ordinary_row_deletes(table, &[1], &mut budget(), &|| false),
        Err(EditError::Conflict)
    ));
    let requests = [RowDelete::new(0, raw.clone())];
    let plan = image(before)
        .plan_ordinary_row_deletes(table, &requests, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.before_bytes(), before);
    assert_eq!(plan.after_bytes(), after);
    assert_eq!(plan.changed_rows(), 1);

    let empty = image(before)
        .plan_ordinary_row_deletes(table, &[], &mut budget(), &|| false)
        .unwrap();
    assert_eq!(empty.after_bytes(), before);
    assert_eq!(empty.changed_rows(), 0);

    let duplicate = [
        RowDelete::new(0, raw.clone()),
        RowDelete::new(0, raw.clone()),
    ];
    assert!(
        image(before)
            .plan_ordinary_row_deletes(table, &duplicate, &mut budget(), &|| false)
            .is_err()
    );
    let mut stale = raw;
    stale[2] ^= 1;
    assert!(matches!(
        image(before).plan_ordinary_row_deletes(
            table,
            &[RowDelete::new(0, stale)],
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Conflict)
    ));

    // A well-framed candidate with one altered retained record byte cannot
    // become authority, even though the only logical row has been removed.
    let mut forged = after;
    forged[data * 4096 + 4095] ^= 1;
    let forged = NativeEditPlan {
        before: before.to_vec(),
        after: forged,
        changed_rows: 1,
    };
    assert!(proof::prove(&forged, scope(), table, &requests, &mut budget(), &|| false).is_err());
}

#[test]
fn original_request_metadata_keeps_duplicates_and_payloads_explicit() {
    let requests = [RowDelete::new(0, vec![1, 2]), RowDelete::new(0, vec![3, 4])];
    assert!(crate::table::mutation::rows::unique(&requests, &mut budget(), &|| false).is_err());
    assert_eq!(requests[0].expected_bytes(), [1, 2]);
    assert_eq!(requests[1].expected_bytes(), [3, 4]);
}

#[test]
fn global_closed_content_tombstone_scope_is_not_silently_expanded() {
    let original = model::source(Case::Named);
    let table = &original.tables[0];
    assert!(
        image(&original.bytes)
            .plan_ordinary_row_deletes(&table.name, &[], &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
#[ignore = "requires the two explicit immutable genuine property/OLE originals"]
fn genuine_property_ole_payload_survives_row_deletion_exactly() {
    let directory = std::env::var_os("TABLECRAFT_GENUINE_PROPERTY_LVAL_SOURCES_DIR").unwrap();
    let directory = Path::new(&directory);
    assert!(directory.is_absolute());
    let cases = [
        (
            "h5.accdb",
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            107,
            110,
        ),
        (
            "h6.accdb",
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            125,
            127,
        ),
    ];
    // Verify both original inputs before invoking any proposed planner.
    let originals: Vec<_> = cases
        .iter()
        .map(|(file, digest, td, data)| {
            let bytes = std::fs::read(directory.join(file)).unwrap();
            let actual = Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(&actual, digest);
            (bytes, *td, *data)
        })
        .collect();
    for (bytes, definition, data) in originals {
        exercise(&bytes, "Sentinel", definition, data);
    }
}
