//! Independently composed two-owner contraction after automatic row expansion.
use super::{
    NEXT, PAGE_BYTES, ROW_COUNT, SLOT_START, budget, capture, checked, database, map_start, model,
    requests, rows, table,
};
use std::io::Cursor;
use tablecraft_ace::{
    Ace16DeletePreview, EditError, RowDelete, TableRowDeletes, inspect_table_indexes,
};
use tablecraft_model::CatalogScope;

const PHYSICAL_INDEX_BYTES: usize = 52;
const INDEX_FLAGS: usize = 46;
const CASCADE_DELETE_FLAGS: u32 = 0x1000;
const CASCADE_DELETE_ACTION: u8 = 1;
const PRIMARY_KIND: u8 = 1;
const PRIMARY_FLAGS: u8 = 0x89;
const RETIRED: [usize; 3] = [26, 27, 28];

fn cascade_database(code: u8) -> model::Model {
    let mut model = database(code);
    model::relationships(&mut model, CASCADE_DELETE_FLAGS);

    let parent = model::definition(1) * PAGE_BYTES;
    let physical = model.logical_start - PHYSICAL_INDEX_BYTES;
    model.bytes[parent + physical + INDEX_FLAGS] = PRIMARY_FLAGS;
    model.bytes[parent + model.logical_start + model::INDEX_KIND] = PRIMARY_KIND;

    let child_link = model::definition(0) * PAGE_BYTES + model.logical_start;
    let parent_link = parent + model.logical_start + model::LOGICAL_BYTES;
    model.bytes[child_link + model::DELETE_ACTION] = CASCADE_DELETE_ACTION;
    model.bytes[parent_link + model::DELETE_ACTION] = CASCADE_DELETE_ACTION;

    checked(&model.bytes);
    let indexes = inspect_table_indexes(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        table(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(indexes.indexes().iter().any(|index| {
        index.native_type() == PRIMARY_KIND
            && index.flags() == PRIMARY_FLAGS
            && index.constraints().is_some_and(|rules| rules.primary_key())
    }));
    model
}

fn verify(model: &model::Model, candidate: &Ace16DeletePreview) {
    let after = candidate.candidate_bytes();
    assert_eq!(candidate.before_bytes(), model.bytes);
    assert_eq!(candidate.deleted_rows(), 5);
    assert_eq!(after.len(), model.bytes.len());
    assert_eq!(rows(after, table(0)), model.rows[0][2..]);
    assert!(rows(after, table(1)).is_empty());
    assert_eq!(rows(after, table(2)), model.rows[2]);
    checked(after);

    let global = map_start(&model.bytes, 1);
    assert_eq!(model.bytes[global + 3], 0xe0);
    assert_eq!(after[global + 3], 0xfc);
    let mut allowed = vec![false; after.len()];
    allowed[global + 3] = true;

    for target in 0..2 {
        let root = model::root(target);
        allowed[root * PAGE_BYTES..(root + 1) * PAGE_BYTES].fill(true);
        let count = model::definition(target) * PAGE_BYTES + ROW_COUNT;
        allowed[count..count + 4].fill(true);
        let owned = map_start(&model.bytes, model::definition(target) + 3);
        allowed[owned + 3] = true;

        for (ordinal, record) in model.records[target].iter().enumerate() {
            let slot = model::data(target) * PAGE_BYTES + SLOT_START + ordinal * 2;
            let original = u16::from_le_bytes(model.bytes[slot..slot + 2].try_into().unwrap());
            let removed = target == 1 || ordinal < 2;
            let expected = if removed { original | 0xc000 } else { original };
            assert_eq!(&after[slot..slot + 2], &expected.to_le_bytes());
            if removed {
                allowed[slot..slot + 2].fill(true);
            }

            let start = model::data(target) * PAGE_BYTES + usize::from(original & 0x1fff);
            assert_eq!(&after[start..start + record.len()], record);
        }
    }

    // The NULL-only child leaf survives; its former next neighbor is retired.
    let retained_leaf = 25 * PAGE_BYTES;
    allowed[retained_leaf + NEXT..retained_leaf + NEXT + 4].fill(true);
    assert_eq!(
        &after[retained_leaf + NEXT..retained_leaf + NEXT + 4],
        &[0; 4]
    );
    for number in RETIRED {
        allowed[number * PAGE_BYTES..number * PAGE_BYTES + 4].fill(true);
        assert_eq!(
            &after[number * PAGE_BYTES..number * PAGE_BYTES + 4],
            &[0; 4]
        );
    }

    for (offset, (&before, &actual)) in model.bytes.iter().zip(after).enumerate() {
        if !allowed[offset] {
            assert_eq!(actual, before, "unexpected mutation at {offset}");
        }
    }
}

#[test]
fn automatic_descendants_retire_two_owners_in_one_global_byte_and_preserve_null_children() {
    for code in [5, 6] {
        let model = cascade_database(code);
        let parents = requests(&model, 1, &[0, 1, 2]);
        let groups = [TableRowDeletes::new(table(1), &parents)];
        let candidate = capture(&model.bytes)
            .preview_ace16_cascading_deletes(&groups, &mut budget(), &|| false)
            .unwrap();
        verify(&model, &candidate);
    }
}

#[test]
fn explicit_descendant_overlap_deduplicates_but_stale_explicit_payloads_still_refuse() {
    let model = cascade_database(6);
    let parents = requests(&model, 1, &[0, 1, 2]);
    let child = requests(&model, 0, &[0]);
    let mut outputs = Vec::new();
    for reverse in [false, true] {
        let mut groups = [
            TableRowDeletes::new(table(1), &parents),
            TableRowDeletes::new(table(0), &child),
        ];
        if reverse {
            groups.reverse();
        }
        let candidate = capture(&model.bytes)
            .preview_ace16_cascading_deletes(&groups, &mut budget(), &|| false)
            .unwrap();
        verify(&model, &candidate);
        outputs.push(candidate.candidate_bytes().to_vec());
    }
    assert_eq!(outputs[0], outputs[1]);

    let stale = [RowDelete::new(0, vec![0])];
    let groups = [
        TableRowDeletes::new(table(1), &parents),
        TableRowDeletes::new(table(0), &stale),
    ];
    assert_eq!(
        capture(&model.bytes)
            .preview_ace16_cascading_deletes(&groups, &mut budget(), &|| false)
            .err(),
        Some(EditError::Conflict)
    );
}

#[test]
fn automatic_row_expansion_cannot_retire_a_page_already_marked_globally_free() {
    let model = cascade_database(6);
    let parents = requests(&model, 1, &[0, 1, 2]);
    let groups = [TableRowDeletes::new(table(1), &parents)];
    let mut damaged = model.bytes.clone();
    let global = map_start(&damaged, 1);
    damaged[global + 26 / 8] |= 1 << (26 % 8);

    assert!(
        capture(&damaged)
            .preview_ace16_cascading_deletes(&groups, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(model.bytes[global + 3], 0xe0);
    let retry = capture(&model.bytes)
        .preview_ace16_cascading_deletes(&groups, &mut budget(), &|| false)
        .unwrap();
    verify(&model, &retry);
}
