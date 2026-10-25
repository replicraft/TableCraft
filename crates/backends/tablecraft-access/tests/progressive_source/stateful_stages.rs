//! Native cursors feed nested barriers without a complete snapshot cache.
use super::*;
use std::ops::Range;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;

const INPUT_ROWS: usize = 1_100;
const PROFILE_OFFSET: usize = 20;
const ENGINE_SIGNATURE: Range<usize> = 4..20;

fn database(profile: u8) -> Vec<u8> {
    let mut bytes = map_fixture::database(INPUT_ROWS, true, false);
    bytes[PROFILE_OFFSET] = profile;
    if profile != 1 {
        bytes[ENGINE_SIGNATURE].copy_from_slice(b"Standard ACE DB\0");
    }
    bytes
}

#[test]
fn nested_groups_exceed_snapshot_limits_and_count_only_physical_input_rows() {
    let syntax = parse_query(
        "SELECT Sum(N) AS Total,Count(*) AS [Groups] FROM \
         (SELECT Id Mod 17 AS K,Count(*) AS N FROM Items GROUP BY Id Mod 17) Q",
        Default::default(),
    )
    .unwrap();
    for profile in [1, 2, 5, 6] {
        let native = source(watched(database(profile)), 3);
        let plan = prepare(syntax.query(), &native).unwrap();
        for declared in [false, true] {
            let mut actual = Vec::new();
            let summary = plan
                .execute_progressive(
                    &native,
                    &[],
                    ExecutionOptions {
                        declared_integral: declared,
                        ..Default::default()
                    },
                    &mut |row| {
                        actual.push(row.to_vec());
                        Ok::<_, ()>(())
                    },
                )
                .unwrap();
            assert_eq!(actual, vec![vec![Value::Int64(1_100), Value::Int64(17)]]);
            assert_eq!(summary.scanned_rows, 1_100);
            assert_eq!(summary.emitted_rows, 1);
        }
        assert_eq!(
            native.scan("Items", &mut |_| Ok(())),
            Err(QueryError::StructureLimit)
        );
    }
}

#[test]
fn late_physical_failures_do_not_finalize_any_downstream_barrier() {
    for bad_count in [false, true] {
        let mut bytes = database(6);
        if bad_count {
            support::put32(
                &mut bytes,
                TABLE_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET,
                INPUT_ROWS as u32 + 1,
            );
        }
        let input = watched(bytes);
        if !bad_count {
            input
                .failed_page
                .set(Some(map_fixture::FIRST_DATA_PAGE + INPUT_ROWS - 1));
        }
        let native = source(input, 3);
        let syntax = parse_query(
            "SELECT Sum(N) AS Total FROM \
             (SELECT Id Mod 17 AS K,Count(*) AS N FROM Items GROUP BY Id Mod 17) Q",
            Default::default(),
        )
        .unwrap();
        let plan = prepare(syntax.query(), &native).unwrap();
        let mut published = 0;
        assert!(
            plan.execute_progressive(&native, &[], Default::default(), &mut |_| {
                published += 1;
                Ok::<_, ()>(())
            })
            .is_err()
        );
        assert_eq!(published, 0);
    }
}
