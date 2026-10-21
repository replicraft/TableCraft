use super::criteria::{run_text, scored};
use super::*;

#[test]
fn interleaved_members_combine_child_alignment_and_independent_parent_folds() {
    let source = scored();
    let query = parse_query("SELECT Files.FileName FROM Items WHERE Other.Label='y' AND Files.Score=0 AND Files.FileType='png'", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&super::criteria::Ordinal),
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        [vec![text("b")]]
    );
    assert_eq!(source.scans.get(), 1);
    let sql =
        "SELECT Files.FileName,Files.FileType FROM Items WHERE Other.Label='x' OR Other.Label='y'";
    assert_eq!(
        run_text(sql, &source).unwrap().rows(),
        [vec![text("a"), text("txt")], vec![text("b"), text("png")]]
    );
    assert_eq!(
        run(
            "SELECT Files.FileName FROM Items WHERE Other.Label IS NULL",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Null]]
    );
    let sql = "SELECT A.Files.FileName,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id WHERE B.Other.Label='y' AND A.Files.FileType='txt'";
    assert_eq!(
        run_text(sql, &source).unwrap().rows(),
        [vec![text("a"), Value::Int32(1)]]
    );
    let sql = "SELECT Files.FileName,Count(*) FROM Items WHERE Other.Label='y' GROUP BY Files.FileName HAVING Count(Files.FileType)>0 ORDER BY Files.FileName";
    assert_eq!(
        run_text(sql, &source).unwrap().rows(),
        [
            vec![text("a"), Value::Int64(1)],
            vec![text("b"), Value::Int64(1)]
        ]
    );
}

#[test]
fn mixed_provider_alignment_and_late_carrier_failures_are_latched() {
    let mut source = scored();
    source.ignore_errors = true;
    source.lists[0][3].pop();
    let sql = "SELECT TOP 1 Files.FileName FROM Items WHERE False AND Files.Score=1 AND Other.Label IS NULL";
    assert_eq!(run(sql, &source).err(), Some(QueryError::Source));
    source.lists[0][3].push(Value::Int32(0));
    source.lists[2][1].push(Value::Int32(12));
    assert!(matches!(
        run(
            "SELECT TOP 1 Files.FileName FROM Items WHERE False AND Other.Label IS NULL",
            &source
        ),
        Err(QueryError::Schema(_))
    ));
}
