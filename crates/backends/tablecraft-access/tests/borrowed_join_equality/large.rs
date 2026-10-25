use super::base as fixture;
use super::*;
#[path = "../../../../engines/tablecraft-ace/tests/large_maps/fixture.rs"]
mod maps;

const JOIN_ROWS: usize = 1_100;

#[test]
fn native_integral_self_joins_fit_default_query_work_in_priority_profiles() {
    let expected = (1..=JOIN_ROWS as i32)
        .map(|id| vec![Value::Int32(id), Value::Int32(id * 10)])
        .collect::<Vec<_>>();

    for profile in [1, 2, 5, 6] {
        let mut bytes = maps::database(JOIN_ROWS, true, false);
        bytes[FORMAT_OFFSET] = profile;
        if profile != 1 {
            bytes[MAGIC_OFFSET..FORMAT_OFFSET].copy_from_slice(b"Standard ACE DB\0");
        }
        for declared in [false, true] {
            for equality in ["L.Id=R.Id", "R.Id=L.Id"] {
                let native = source(bytes.clone());
                let sql =
                    format!("SELECT L.Id,R.Score FROM Items L INNER JOIN Items R ON {equality}",);
                let syntax = parse_query(&sql, Default::default()).unwrap();
                let plan = prepare(syntax.query(), &native).unwrap();
                let result = plan
                    .execute(
                        &native,
                        &[],
                        ExecutionOptions {
                            declared_integral: declared,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                assert_eq!(result.rows(), expected);
            }
        }
    }
}
