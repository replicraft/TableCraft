use super::criteria::{run_text, scored};
use super::*;

#[test]
fn named_pivot_siblings_filter_aligned_children_beside_independent_membership() {
    let source = scored();
    let sql = "TRANSFORM Count(*) SELECT Id FROM Items WHERE Files.FileType='txt' GROUP BY Id PIVOT Files.FileName IN ('a','b')";
    assert_eq!(
        run_text(sql, &source).unwrap().rows(),
        [vec![Value::Int32(1), Value::Int64(1), Value::Null]]
    );
    let sql = "TRANSFORM Count(*) SELECT Id FROM Items WHERE Other.Label='x' AND Files.FileType='txt' GROUP BY Id PIVOT Files.FileName";
    let result = run_text(sql, &source).unwrap();
    assert_eq!(result.columns(), ["Id", "a", "b"]);
    assert_eq!(
        result.rows(),
        [vec![Value::Int32(1), Value::Int64(1), Value::Null]]
    );
    let sql = "TRANSFORM Count(*) SELECT Id FROM Items WHERE Other.Label IS NULL GROUP BY Id PIVOT Files.FileName";
    let result = run_text(sql, &source).unwrap();
    assert_eq!(result.columns(), ["Id", "a", "b"]);
    assert_eq!(
        result.rows(),
        [vec![Value::Int32(2), Value::Null, Value::Null]]
    );
    let mut source = scored();
    source.ignore_errors = true;
    source.lists[0][3].pop();
    let sql = "TRANSFORM Count(*) SELECT Id FROM Items WHERE Other.Label IS NULL AND Files.Score=1 GROUP BY Id PIVOT Files.FileName";
    assert_eq!(run_text(sql, &source).err(), Some(QueryError::Source));
}
