use super::criteria::{run_text, scored};
use super::*;

#[test]
fn named_member_subqueries_preserve_interleaved_alignment_and_empty_null_distinctions() {
    let source = scored();
    let sql = "SELECT Id FROM Items WHERE EXISTS(SELECT 1 WHERE Files.FileName IS NULL)";
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [vec![Value::Int32(3)]]
    );
    let sql = "SELECT Files.FileName FROM Items WHERE Other.Label=(SELECT 'y') AND Files.Score=0";
    assert_eq!(run_text(sql, &source).unwrap().rows(), [vec![text("b")]]);
    let sql = "SELECT Files.FileName FROM Items WHERE EXISTS(SELECT 1 WHERE Other.Label IS NULL)";
    assert_eq!(run(sql, &source).unwrap().rows(), [vec![Value::Null]]);
    let source = scored();
    let sql = "SELECT Id FROM Items WHERE EXISTS(SELECT 1 WHERE Files.FileName IS NULL AND Files.Score=1)";
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows()
            .is_empty()
    );
}
