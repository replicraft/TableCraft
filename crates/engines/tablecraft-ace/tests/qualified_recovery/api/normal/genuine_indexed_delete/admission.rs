//! Negative original derivatives; no new healthy-tree or AFTER qualification.
use super::{budget, fixtures, image, scope};
use fixtures::Recipe;
use std::io::Cursor;
use tablecraft_ace::{RowDelete, read_table};
use tablecraft_testkit::fixtures::access::genuine_indexed_delete as sources;

fn refused_before_plan(original: &[u8], changed: &[u8]) {
    assert_ne!(changed, original);
    // Only target metadata/INDEX framing changes. The complete scalar rows stay
    // readable through the independent sequential channel before admission.
    let snapshot = read_table(
        &mut Cursor::new(changed),
        scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(snapshot.rows(), sources::original_rows());

    for requests in [
        Vec::new(),
        vec![RowDelete::new(
            0,
            sources::witnesses(Recipe::First, false).remove(0).1,
        )],
    ] {
        let ordinals = requests
            .iter()
            .map(|request| request.row())
            .collect::<Vec<_>>();
        assert!(
            image(changed)
                .prepare_ordinary_row_deletes("Items", &ordinals, &mut budget(), &|| false,)
                .is_err()
        );
        assert!(
            image(changed)
                .plan_recoverable_ordinary_row_deletes("Items", requests, &mut budget(), &|| false,)
                .is_err()
        );
    }
}

#[test]
#[ignore = "requires all sixteen source-first indexed DELETE views"]
fn unsupported_flags_and_distinct_physical_owners_cannot_hide_behind_empty_requests() {
    for case in fixtures::cases()
        .into_iter()
        .filter(|case| case.recipe == Recipe::First)
    {
        let td = case.definition * 4096;
        let secondary = td + 244;
        assert_eq!(case.before[secondary + 46], 0x80);
        assert_eq!(
            &case.before[secondary + 4..secondary + 6],
            &2_u16.to_le_bytes()
        );
        for flags in [0x82, 0x84] {
            // Known IGNORE_NULLS (currently outside this action) and unknown
            // physical flags are distinct refusal causes; neither is permission.
            let mut changed = case.before.clone();
            changed[secondary + 46] = flags;
            refused_before_plan(&case.before, &changed);
        }

        // Compatible logical aliases share one physical number. This negative
        // instead gives TWO physical descriptors the same actual root/map/key.
        // Both key populations are individually meaningful Id populations;
        // the exclusive physical owner census must still reject the overlap.
        let mut overlap = case.before.clone();
        let root = u32::try_from(case.roots[0]).unwrap();
        let primary_usage = ((u32::try_from(case.definition).unwrap() + 1) << 8) | 2;
        assert_eq!(
            &case.before[td + 226..td + 230],
            &primary_usage.to_le_bytes()
        );
        overlap[secondary + 4..secondary + 6].copy_from_slice(&0_u16.to_le_bytes());
        overlap[secondary + 34..secondary + 38].copy_from_slice(&primary_usage.to_le_bytes());
        overlap[secondary + 38..secondary + 42].copy_from_slice(&root.to_le_bytes());
        refused_before_plan(&case.before, &overlap);
    }
}

#[test]
#[ignore = "requires all sixteen source-first indexed DELETE views"]
fn unsupported_leaf_write_stamp_and_links_refuse_even_when_no_rows_are_selected() {
    for case in fixtures::cases()
        .into_iter()
        .filter(|case| case.recipe == Recipe::First)
    {
        let base = case.roots[0] * 4096;
        assert_eq!(&case.before[base..base + 2], &[4, 1]);
        assert_eq!(&case.before[base + 8..base + 27], &[0; 19]);
        for offset in [8, 12, 16, 20] {
            // These are explicit framing negatives, not fabricated healthy NODE
            // source images. Scalar DATA remains untouched and readable.
            let mut changed = case.before.clone();
            changed[base + offset] = 1;
            refused_before_plan(&case.before, &changed);
        }
    }
}
