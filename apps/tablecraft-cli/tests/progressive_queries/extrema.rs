use super::*;

const PROFILE_OFFSET: usize = 20;
const FORMAT_NAME_RANGE: std::ops::Range<usize> = 4..20;

#[test]
fn global_extrema_compose_with_other_aggregates_across_priority_profiles() {
    for profile in [1, 2, 5, 6] {
        let mut bytes = fixture::database();
        bytes[PROFILE_OFFSET] = profile;
        if profile != 1 {
            bytes[FORMAT_NAME_RANGE].copy_from_slice(b"Standard ACE DB\0");
        }
        let file = FileCopy::new(bytes);
        let sql = concat!(
            "SELECT Min(Id) AS Low,Max(Score) AS High,Count(*) AS N,",
            "Sum(Id) AS Total,Avg(Id) AS Mean FROM Items",
        );
        let expected = b"Low\tHigh\tN\tTotal\tMean\n1\t30\t3\t6\t2\n";
        assert_eq!(success(file.query(sql, &[])), expected);
        for batch in ["1", "2"] {
            assert_eq!(
                success(file.query(sql, &["--stream", "--batch-rows", batch])),
                expected
            );
        }
    }
}

#[test]
fn progressive_text_extrema_use_the_same_explicit_comparer_and_formatting() {
    let file = FileCopy::new(fixture::database());
    let sql = "SELECT Min(CStr(Id)) AS Low,Max(CStr(Score)) AS High FROM Items";
    let options = ["--number-format", "invariant", "--collation", "ordinal"];
    let expected = b"Low\tHigh\n\"\"\"1\"\"\"\t\"\"\"30\"\"\"\n";
    assert_eq!(success(file.query(sql, &options)), expected);

    let mut streaming = options.to_vec();
    streaming.extend(["--stream", "--batch-rows", "1"]);
    assert_eq!(success(file.query(sql, &streaming)), expected);
}

#[test]
fn retained_extrema_are_observable_before_strict_scalar_publication() {
    let file = FileCopy::new(fixture::database());
    let sql = concat!(
        "SELECT LenB(Min(ChrW(55296))) AS Units,LenB(Max(ChrB(65))) AS Bytes,",
        "CLng(Max(CVErr(Id))) AS Code FROM Items WHERE Id=1",
    );
    let expected = b"Units\tBytes\tCode\n2\t1\t1\n";
    assert_eq!(success(file.query(sql, &[])), expected);
    assert_eq!(success(file.query(sql, &["--stream"])), expected);
    assert!(
        failure(file.query(
            "SELECT Min(ChrW(55296)) FROM Items WHERE Id=1",
            &["--stream"],
        ))
        .is_empty()
    );
}

#[test]
fn extrema_failures_preserve_only_completed_earlier_union_batches() {
    let file = FileCopy::new(fixture::database());
    let sql = concat!(
        "SELECT Count(*) AS N FROM Items UNION ALL ",
        "SELECT Min(10\\(Id-2)) FROM Items UNION ALL SELECT Id FROM Items",
    );
    assert_eq!(
        failure(file.query(sql, &["--stream", "--batch-rows", "1"])),
        b"N\n3\n"
    );
    assert!(failure(file.query(sql, &["--stream", "--batch-rows", "2"])).is_empty());

    let empty = "SELECT Min(Id) AS Low,Max(Score) AS High FROM Items WHERE False";
    assert_eq!(
        success(file.query(empty, &["--stream"])),
        b"Low\tHigh\nNULL\tNULL\n"
    );
}
