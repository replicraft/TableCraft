use super::*;
use std::ops::Range;
use tablecraft_core::values::Double;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;

const INPUT_ROWS: usize = 1_100;
const GROUPS: i32 = 17;
const PROFILE_OFFSET: usize = 20;
const FORMAT_RANGE: Range<usize> = 4..20;

fn expected() -> Vec<Vec<Value>> {
    let mut groups: Vec<(i32, Vec<i32>)> = Vec::new();
    for id in 1..=INPUT_ROWS as i32 {
        let key = id % GROUPS;
        if let Some((_, values)) = groups.iter_mut().find(|(found, _)| *found == key) {
            values.push(id);
        } else {
            groups.push((key, vec![id]));
        }
    }
    groups
        .into_iter()
        .map(|(key, values)| {
            let count = values.len() as i64;
            let sum: i64 = values.iter().map(|value| i64::from(*value)).sum();
            vec![
                Value::Int32(key),
                Value::Int64(count),
                Value::Int64(sum * 10),
                Value::Double(Double::from_float(sum as f64 / count as f64)),
                Value::Int32(*values.iter().min().unwrap()),
                Value::Int32(values.iter().max().unwrap() * 10),
            ]
        })
        .collect()
}

#[test]
fn grouped_native_cursors_exceed_snapshot_retention_without_publishing_a_cache() {
    let expected = expected();
    let sql = concat!(
        "SELECT CLng(Id Mod 17),Count(*),Sum(Score),Avg(Id),Min(Id),Max(Score) ",
        "FROM Items GROUP BY Id Mod 17",
    );
    for profile in [1, 2, 5, 6] {
        for batch in [1, 3] {
            let mut bytes = map_fixture::database(INPUT_ROWS, true, false);
            bytes[PROFILE_OFFSET] = profile;
            if profile != 1 {
                bytes[FORMAT_RANGE].copy_from_slice(b"Standard ACE DB\0");
            }
            let native = source(watched(bytes), batch);
            let syntax = parse_query(sql, Default::default()).unwrap();
            let plan = prepare(syntax.query(), &native).unwrap();
            for declared in [false, true] {
                let mut rows = Vec::new();
                plan.execute_progressive(
                    &native,
                    &[],
                    ExecutionOptions {
                        declared_integral: declared,
                        ..Default::default()
                    },
                    &mut |row| {
                        rows.push(row.to_vec());
                        Ok::<(), ()>(())
                    },
                )
                .unwrap();
                assert_eq!(rows, expected);
            }
            assert_eq!(
                native.scan("Items", &mut |_| Ok(())),
                Err(QueryError::StructureLimit)
            );
        }
    }
}

#[test]
fn checked_native_eof_precedes_every_group_result_even_with_top_one() {
    for corrupt_count in [false, true] {
        let mut bytes = map_fixture::database(INPUT_ROWS, true, false);
        if corrupt_count {
            support::put32(
                &mut bytes,
                TABLE_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET,
                INPUT_ROWS as u32 + 1,
            );
        }
        let input = watched(bytes);
        if !corrupt_count {
            input
                .failed_page
                .set(Some(map_fixture::FIRST_DATA_PAGE + INPUT_ROWS - 1));
        }
        let native = source(input, 3);
        let syntax = parse_query(
            "SELECT TOP 1 Id Mod 17,Count(*) FROM Items GROUP BY Id Mod 17",
            Default::default(),
        )
        .unwrap();
        let plan = prepare(syntax.query(), &native).unwrap();
        let mut published = 0;
        assert!(
            plan.execute_progressive(&native, &[], Default::default(), &mut |_| {
                published += 1;
                Ok::<(), ()>(())
            })
            .is_err()
        );
        assert_eq!(published, 0);
    }
}
