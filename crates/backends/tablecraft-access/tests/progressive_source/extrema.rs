use super::*;
use tablecraft_core::values::Double;
use tablecraft_query::{ExecutionOptions, StreamError, prepare};
use tablecraft_sql::parse_query;

const PROFILE_OFFSET: usize = 20;
const FORMAT_NAME_RANGE: std::ops::Range<usize> = 4..20;
const USAGE_PAGE: u32 = 11;
const DATA_PAGE: usize = 12;

fn image(profile: u8) -> Vec<u8> {
    let mut bytes = map_fixture::database(7, true, false);
    bytes[PROFILE_OFFSET] = profile;
    if profile != 1 {
        bytes[FORMAT_NAME_RANGE].copy_from_slice(b"Standard ACE DB\0");
    }
    bytes
}

fn execute(source: &impl Source, sql: &str) -> Result<Vec<Vec<Value>>, StreamError<()>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), source).unwrap();
    let mut rows = Vec::new();
    plan.execute_progressive(source, &[], ExecutionOptions::default(), &mut |row| {
        rows.push(row.to_vec());
        Ok(())
    })?;
    Ok(rows)
}

#[test]
fn extrema_cross_priority_native_batches_without_complete_snapshot_publication() {
    for profile in [1, 2, 5, 6] {
        for batch in [1, 3] {
            let source = source(watched(image(profile)), batch);
            let rows = execute(
                &source,
                "SELECT Min(Id),Max(Score),Count(*),Sum(Id),Avg(Id) FROM Items",
            )
            .unwrap();
            assert_eq!(
                rows,
                [vec![
                    Value::Int32(1),
                    Value::Int32(70),
                    Value::Int64(7),
                    Value::Int64(28),
                    Value::Double(Double::from_float(4.0)),
                ]]
            );
            assert_eq!(
                source.scan("Items", &mut |_| Ok(())),
                Err(QueryError::StructureLimit)
            );
            assert_eq!(
                execute(&source, "SELECT Min(Id),Max(Id) FROM Items").unwrap(),
                [vec![Value::Int32(1), Value::Int32(7)]]
            );
        }
    }
}

#[test]
fn extrema_require_checked_native_eof_before_any_result_callback() {
    for corrupt_count in [false, true] {
        let mut bytes = image(6);
        if corrupt_count {
            support::put32(&mut bytes, TABLE_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET, 8);
        }
        let input = watched(bytes);
        if !corrupt_count {
            input
                .failed_page
                .set(Some(map_fixture::FIRST_DATA_PAGE + 6));
        }
        let source = source(input, 1);
        let syntax =
            parse_query("SELECT Min(Id),Max(Score) FROM Items", Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let mut visits = 0;
        let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
            visits += 1;
            Ok::<(), ()>(())
        });
        assert!(result.is_err());
        assert_eq!(visits, 0);
    }
}

#[test]
fn selected_native_payloads_validate_before_filters_while_unselected_payloads_stay_pruned() {
    let mut bytes = fixture::database();
    bytes[TABLE_PAGE * PAGE_BYTES..(TABLE_PAGE + 1) * PAGE_BYTES].copy_from_slice(
        &fixture::definition(&[("Id", 4, 0, 4), ("BadDate", 8, 4, 8)], 3, USAGE_PAGE << 8),
    );
    let rows = (1_i32..=3)
        .map(|id| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(id.to_le_bytes());
            row.extend(f64::NAN.to_bits().to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(TABLE_PAGE as u32, &rows));

    let source = source(watched(bytes), 1);
    assert_eq!(
        execute(&source, "SELECT Min(Id),Max(Id) FROM Items").unwrap(),
        [vec![Value::Int32(1), Value::Int32(3)]]
    );
    assert!(execute(&source, "SELECT Min(BadDate) FROM Items WHERE False").is_err());
}
