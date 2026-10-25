use super::*;

#[test]
fn global_numeric_aggregates_compose_with_counts_aliases_and_union_arms() {
    let file = FileCopy::new(fixture::database());
    for (sql, expected) in [
        (
            "SELECT Sum(Id) AS Total,Avg(Id) AS Mean,Count(Id) AS N,Total/CLngLng(2) AS Half FROM Items",
            b"Total\tMean\tN\tHalf\n6\t2\t3\t3\n".as_slice(),
        ),
        (
            "SELECT Id AS N FROM Items WHERE Id=1 UNION ALL SELECT Sum(Id) FROM Items UNION ALL SELECT Avg(Id) FROM Items",
            b"N\n1\n6\n2\n".as_slice(),
        ),
        (
            "SELECT Sum(Id) AS Total,Avg(Id) AS Mean,Count(*) AS N FROM Items WHERE False",
            b"Total\tMean\tN\nNULL\tNULL\t0\n".as_slice(),
        ),
    ] {
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
fn numeric_aggregate_failures_publish_only_completed_previous_arm_batches() {
    let file = FileCopy::new(fixture::database());
    for sql in [
        "SELECT Sum(10\\(Id-2)) FROM Items",
        "SELECT Avg(IIf(True,Id,1/0)) FROM Items",
        "SELECT Sum(ChrW(55296)) FROM Items",
        "SELECT Avg(CVErr(2001)) FROM Items",
    ] {
        assert!(failure(file.query(sql, &[])).is_empty());
        assert!(failure(file.query(sql, &["--stream", "--batch-rows", "1"])).is_empty());
    }

    let sql = concat!(
        "SELECT Count(*) AS N FROM Items UNION ALL ",
        "SELECT Sum(10\\(Id-2)) FROM Items",
    );
    assert_eq!(
        failure(file.query(sql, &["--stream", "--batch-rows", "1"])),
        b"N\n3\n"
    );
    assert!(failure(file.query(sql, &["--stream", "--batch-rows", "2"])).is_empty());
}

#[test]
fn opted_in_numeric_aggregates_consume_stored_caches_without_executing_formulas() {
    let file = FileCopy::new(calculated_fixture::database_with_expression(
        &[Some(999), None, Some(41)],
        "1/0",
    ));
    let sql = "SELECT Sum(Score) AS Total,Avg(Score) AS Mean,Count(Score) AS N FROM Items";
    let expected = b"Total\tMean\tN\n1040\t520\t2\n";

    assert!(failure(file.query(sql, &["--stream"])).is_empty());
    assert_eq!(
        success(file.query(sql, &["--stored-calculations"])),
        expected
    );
    assert_eq!(
        success(file.query(
            sql,
            &["--stream", "--stored-calculations", "--batch-rows", "1"],
        )),
        expected
    );
}
