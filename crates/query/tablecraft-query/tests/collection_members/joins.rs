use super::criteria::{run_text, scored};
use super::*;

#[test]
fn interleaved_on_members_drive_sibling_alignment_and_independent_where_membership() {
    let source = scored();
    let sql = "SELECT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Files.Score=B.Id-1 ORDER BY B.Id";
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [
            vec![Value::Int32(1), Value::Int32(1)],
            vec![Value::Int32(1), Value::Int32(2)]
        ]
    );
    let sql = "SELECT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Files.Score=B.Id-1 WHERE A.Files.FileType='png'";
    assert_eq!(
        run_text(sql, &source).unwrap().rows(),
        [vec![Value::Int32(1), Value::Int32(1)]]
    );
    let sql = "SELECT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Files.Score=B.Id-1 WHERE A.Other.Label='y'";
    assert_eq!(run_text(sql, &source).unwrap().rows().len(), 2);
    let mut source = scored();
    source.ignore_errors = true;
    source.lists[0][3].pop();
    let sql = "SELECT TOP 1 A.Id FROM Items AS A INNER JOIN Items AS B ON False AND A.Files.Score=B.Id AND A.Files.FileType='txt'";
    assert_eq!(run_text(sql, &source).err(), Some(QueryError::Source));
}
