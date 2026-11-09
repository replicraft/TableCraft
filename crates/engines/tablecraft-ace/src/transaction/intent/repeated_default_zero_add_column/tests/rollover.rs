//! Resizable V2 replay remains bound to complete native rollover reconstruction.
use super::*;

fn source() -> Vec<u8> {
    let mut before = raw::source(Source::IdOnly);
    for number in 2..=13 {
        before = raw::append(&before, &format!("N{number}"));
    }
    before
}

#[test]
fn one_page_growth_commits_or_rolls_back_without_changing_the_frozen_intent() {
    let before = source();
    let after = raw::append(&before, "N14");
    assert_eq!(after.len(), before.len() + 4096);
    let wire = literal(&before, "Table1", "N14");
    for committed in [false, true] {
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = plan(&before, "Table1", "N14")
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
            // A crash can leave the EOF growth and only some old-page edits.
            target.bytes.get_mut().resize(after.len(), 0);
            target.bytes.get_mut()[11 * 4096..].copy_from_slice(&after[11 * 4096..]);
            target.bytes.get_mut()[3 * 4096..4 * 4096].copy_from_slice(&after[3 * 4096..4 * 4096]);
        }
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_repeated_default_zero_add_column_recovery(
            checked,
            decode(&wire),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let expected = if committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        };
        assert_eq!(
            qualified
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap(),
            expected
        );
        assert_eq!(
            target.bytes.get_ref(),
            if committed { &after } else { &before }
        );
        let writes = target.writes;
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let settled = qualify_native_repeated_default_zero_add_column_recovery(
            checked,
            decode(&wire),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .confirm_completed(&mut target, &mut budget(), &|| false)
        .unwrap();
        assert_eq!(settled, expected);
        assert_eq!(target.writes, writes);
    }
}

#[test]
fn a_valid_generic_v2_digest_does_not_authorize_foreign_alias_map_edits() {
    let before = source();
    let expected = raw::append(&before, "N14");
    // Independent native map cells: another catalog-owned map, the global-free
    // map and the selected owned-LVAL map cannot be altered beyond the exact
    // reconstructed transition. Each still has a valid generic V2 digest.
    for offset in [3 * 4096 + 4089, 3 * 4096 + 4076, 3 * 4096 + 3881] {
        let mut after = expected.clone();
        after[offset] ^= 16;
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_expected(
                &mut target,
                &mut journal,
                &before,
                &after,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(
            qualify_native_repeated_default_zero_add_column_recovery(
                checked,
                decode(&literal(&before, "Table1", "N14")),
                SCOPE,
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &before);
    }
}
