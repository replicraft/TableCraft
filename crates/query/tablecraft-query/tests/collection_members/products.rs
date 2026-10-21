use super::criteria::{run_text, scored};
use super::*;
use proptest::prelude::*;

#[test]
fn members_of_one_hidden_collection_align_but_separate_atoms_remain_independent() {
    let source = scored();
    let sql =
        "SELECT Id FROM Items WHERE EXISTS(SELECT 1 WHERE Files.FileName='a' AND fIlEs.Score=0)";
    assert!(run_text(sql, &source).unwrap().rows().is_empty());
    let sql =
        "SELECT Id FROM Items WHERE EXISTS(SELECT 1 WHERE Files.FileName='a' AND fIlEs.Score=1)";
    assert_eq!(
        run_text(sql, &source).unwrap().rows(),
        [vec![Value::Int32(1)]]
    );
    let sql = "SELECT Id FROM Items WHERE Files.FileName='a' AND fIlEs.Score=0";
    assert_eq!(
        run_text(sql, &source).unwrap().rows(),
        [vec![Value::Int32(1)]]
    );
    let sql = "SELECT Id FROM Items WHERE IIf(Files.FileType='txt',Files.Score=1,False)";
    assert_eq!(
        run_text(sql, &source).unwrap().rows(),
        [vec![Value::Int32(1)]]
    );
    let sql = "SELECT A.Id,B.Id FROM Items AS A,Items AS B WHERE A.Files.Score=B.Files.Score";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [
            vec![Value::Int32(1), Value::Int32(1)],
            vec![Value::Int32(3), Value::Int32(3)]
        ]
    );
}

#[test]
fn hidden_tuple_alignment_and_validation_failures_are_latched_before_empty_folds() {
    let mut source = scored();
    source.ignore_errors = true;
    source.lists[0][3].pop();
    let sql = "SELECT TOP 1 Id FROM Items WHERE EXISTS(SELECT 1 WHERE Files.FileName IS NULL AND Files.Score=1)";
    assert_eq!(run(sql, &source).err(), Some(QueryError::Source));
    source.lists[0][3].push(Value::Int32(0));
    source.lists[0][1].clear();
    source.lists[0][3].push(text("bad"));
    let sql =
        "SELECT TOP 1 Id FROM Items WHERE False AND IIf(Other.Label IS NULL,Files.Score=1,False)";
    assert!(matches!(run(sql, &source), Err(QueryError::Schema(_))));
}

proptest! {
    #![proptest_config(proptest::test_runner::Config { failure_persistence: None, ..Default::default() })]
    #[test]
    fn generated_same_child_comparisons_match_a_zipped_nullable_model(
        parents in prop::collection::vec(prop::collection::vec((
            prop::option::of(0_i32..4), prop::option::of(0_i32..4),
        ),0..5),0..7),
    ) {
        let mut source = scored();
        source.members.push(CollectionMember::new("FILES".into(),Column::new("Second".into(),ScalarType::Int32,true).unwrap()).unwrap());
        source.lists = parents.iter().map(|pairs| vec![
            pairs.iter().map(|_| text("child")).collect(), vec![],
            pairs.iter().map(|_| text("txt")).collect(),
            pairs.iter().map(|(a,_)| a.map_or(Value::Null,Value::Int32)).collect(),
            pairs.iter().map(|(_,b)| b.map_or(Value::Null,Value::Int32)).collect(),
        ]).collect();
        let expected = parents.iter().enumerate()
            .filter(|(_,pairs)| pairs.iter().any(|(a,b)| a.is_some() && a==b))
            .map(|(id,_)| vec![Value::Int32(id as i32+1)]).collect::<Vec<_>>();
        let result = run("SELECT Id FROM Items WHERE Files.Score=Files.[Second]",&source).unwrap();
        prop_assert_eq!(result.rows(),expected);
    }
}
