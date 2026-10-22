use super::criteria::run_text;
use super::*;

#[test]
fn sort_only_named_members_keep_siblings_aligned_and_hidden_collections_independent() {
    let source = Attachments::new();
    assert_eq!(
        run_text("SELECT Id FROM Items ORDER BY Files.FileName,Id", &source)
            .unwrap()
            .rows(),
        &[
            vec![Value::Int32(2)],
            vec![Value::Int32(3)],
            vec![Value::Int32(1)],
            vec![Value::Int32(1)],
        ]
    );
    assert_eq!(
        run_text(
            "SELECT Id FROM Items WHERE Files.FileType='png' ORDER BY Files.FileName",
            &source
        )
        .unwrap()
        .rows(),
        &[vec![Value::Int32(1)]]
    );
    assert_eq!(
        run_text(
            "SELECT Id FROM Items WHERE Other.Label='y' ORDER BY Files.FileName",
            &source
        )
        .unwrap()
        .rows(),
        &[vec![Value::Int32(1)], vec![Value::Int32(1)]]
    );
    assert_eq!(run("SELECT Id,Count(*) FROM Items GROUP BY Id HAVING Count(Files.FileName)>0 ORDER BY Count(Files.FileType)", &source).unwrap().rows(), &[vec![Value::Int32(1), Value::Int64(2)]]);
}

#[test]
fn hidden_named_group_and_sort_members_still_validate_sibling_alignment() {
    for sql in [
        "SELECT Id FROM Items WHERE False AND Files.FileType='txt' ORDER BY Files.FileName",
        "SELECT Id FROM Items GROUP BY Id HAVING Count(Files.FileName)>0 ORDER BY Count(Files.FileType)",
    ] {
        let mut source = Attachments::new();
        source.ignore_errors = true;
        source.lists[0][2].pop();
        assert_eq!(run_text(sql, &source).err(), Some(QueryError::Source));
    }
}
