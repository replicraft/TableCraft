//! Source-local authority survives INDEX routing without accepting new owners.
use super::super::{original, raw};
use super::{
    model::{self, MemoMaps, Row},
    zero_memo::values,
};
use crate::{
    EditError, RowInsert, WriteBlocker, check_table,
    format::reader::Reader,
    table::mutation::{append::State, map_authority::Owner, variable_split::metadata::Metadata},
};
use tablecraft_testkit::access::multiple_indexes as frames;

const PAGE: usize = 4096;

#[test]
fn retained_initialization_authority_is_bound_to_the_actual_source_and_owner() {
    for code in [5, 6] {
        let case = model::source_with_memo_maps(code, true, MemoMaps::Zero);
        values(&case.before, &case.rows);
        let image = original::capture(&case.before);
        let table = check_table(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            "Items",
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
        let metadata = Metadata::capture_for_insert_initialization(
            &image,
            table.schema(),
            table.indexes(),
            &[3],
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
        let mut state = State::new(&image, 10);
        state.retain_source_metadata(&image, metadata).unwrap();
        let owners = [Owner::IndexOwned {
            definition: 10,
            physical: 0,
        }];
        let mut source = image.reader();
        let mut budget = original::budget();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget: &mut budget,
            cancelled: &|| false,
            reads: 0,
        };
        state
            .require_source_map_exclusive(&image, &owners, &mut reader)
            .unwrap();

        // Equal bytes in a separate immutable capture grant no source identity.
        let other = original::capture(&case.before);
        assert_eq!(
            state.require_source_map_exclusive(&other, &owners, &mut reader),
            Err(EditError::Preservation)
        );
        state.owner = 11;
        assert_eq!(
            state.require_source_map_exclusive(&image, &owners, &mut reader),
            Err(EditError::Preservation)
        );
    }
}

#[test]
fn initialization_source_census_keeps_foreign_column_map_sharing_forbidden() {
    for code in [5, 6] {
        let case = model::source_with_memo_maps(code, true, MemoMaps::Zero);
        let mut shared = case.before;
        // Blob's owned map now aliases the primary physical INDEX map row.
        raw::put32(&mut shared, 10 * PAGE + 545, 15 << 8);
        let image = original::capture(&shared);
        let table = check_table(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            "Items",
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            Metadata::capture_for_insert_initialization(
                &image,
                table.schema(),
                table.indexes(),
                &[3],
                &mut original::budget(),
                &|| false,
            ),
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        ));
    }
}

struct Recursive {
    before: Vec<u8>,
    expected: Vec<u8>,
    rows: Vec<(Row, u32)>,
    added: Vec<Row>,
    final_rows: Vec<(Row, u32)>,
}

fn recursive(code: u8) -> Recursive {
    let case = model::source_with_memo_maps(code, true, MemoMaps::Zero);
    let mut before = case.before;
    let keys = case
        .rows
        .iter()
        .map(|(row, reference)| {
            let mut key = vec![127];
            key.extend(((row.id as u32) ^ 0x80000000).to_be_bytes());
            key.extend(reference.to_be_bytes());
            key
        })
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), 400);
    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    before.resize(22 * PAGE, 0);
    frames::install(&mut before, 20, &frames::leaf(None, &keys[..200], 0, 0, 21));
    frames::install(&mut before, 21, &frames::leaf(None, &keys[200..], 0, 20, 0));
    let root = frames::root(&before[13 * PAGE..14 * PAGE], 20, 21, &keys[199]);
    frames::install(&mut before, 13, &root);
    frames::map_page(
        &mut before,
        15,
        vec![frames::map([13, 20, 21]), frames::map([14])],
    );
    frames::map_page(&mut before, 1, vec![frames::map(22..512)]);
    assert_eq!(before[13 * PAGE], 3);
    let mut final_rows = case.rows.clone();
    final_rows.extend(
        case.added
            .iter()
            .cloned()
            .enumerate()
            .map(|(slot, row)| (row, (25 << 8) | slot as u32)),
    );
    let mut expected = before.clone();
    expected.resize(26 * PAGE, 0);
    let mut records = Vec::new();
    for (slot, row) in case.added.iter().enumerate() {
        let memo = model::descriptor(&[0, 1].repeat(33), Some((23 << 8) | slot as u32));
        let blob = model::descriptor(row.blob.as_ref().unwrap(), Some((24 << 8) | slot as u32));
        records.push((0, model::record(row.id, 0, Some(&blob), Some(&memo), false)));
    }
    frames::install(&mut expected, 25, &model::data_page(10, &records));
    frames::install(
        &mut expected,
        23,
        &model::long_page(&vec![[0, 1].repeat(33); 52]),
    );
    frames::install(
        &mut expected,
        24,
        &model::long_page(&vec![vec![0x85; 65]; 52]),
    );
    model::maps(
        &mut expected,
        &[12, 18, 19, 25],
        &[25],
        &[16],
        &[16],
        &[17, 24],
        &[24],
    );
    frames::map_page(
        &mut expected,
        22,
        vec![frames::map([23]), frames::map([23])],
    );
    raw::put16(&mut expected, 22 * PAGE + 2, 3940);
    raw::put32(&mut expected, 10 * PAGE + 535, 22 << 8);
    raw::put32(&mut expected, 10 * PAGE + 539, (22 << 8) | 1);
    raw::put32(&mut expected, 10 * PAGE + 16, 452);
    raw::put32(&mut expected, 10 * PAGE + 67, 469);
    frames::map_page(&mut expected, 1, vec![frames::map(26..512)]);
    let final_primary = model::keys(&final_rows, true);
    frames::install(
        &mut expected,
        21,
        &frames::leaf(
            Some(&before[21 * PAGE..22 * PAGE]),
            &final_primary[200..],
            0,
            20,
            0,
        ),
    );
    frames::install(
        &mut expected,
        14,
        &frames::leaf(
            Some(&before[14 * PAGE..15 * PAGE]),
            &model::keys(&final_rows, false),
            0,
            0,
            0,
        ),
    );
    values(&expected, &final_rows);
    Recursive {
        before,
        expected,
        rows: case.rows,
        added: case.added,
        final_rows,
    }
}

#[test]
fn recursive_zero_memo_sources_and_expected_images_qualify_before_preview() {
    for code in [5, 6] {
        let case = recursive(code);
        values(&case.before, &case.rows);
        values(&case.expected, &case.final_rows);
    }
}

#[test]
fn recursive_index_routing_reuses_the_same_qualified_zero_memo_source() {
    for code in [5, 6] {
        let case = recursive(code);
        values(&case.before, &case.rows);
        values(&case.expected, &case.final_rows);
        let requests = case
            .added
            .iter()
            .map(|row| RowInsert::new(row.values()))
            .collect::<Vec<_>>();
        let candidate = original::capture(&case.before)
            .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
            .unwrap();
        let difference = candidate
            .candidate_bytes()
            .iter()
            .zip(&case.expected)
            .position(|(actual, expected)| actual != expected);
        assert_eq!(candidate.candidate_bytes().len(), case.expected.len());
        assert_eq!(
            difference, None,
            "recursive source/request image difference"
        );
        values(candidate.candidate_bytes(), &case.final_rows);
        assert_eq!(candidate.candidate_bytes()[13 * PAGE], 3);
    }
}
