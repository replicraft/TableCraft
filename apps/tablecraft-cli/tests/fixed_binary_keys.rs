mod support;

use support::run;

#[test]
fn mixed_numeric_groups_and_large_integer_sorting_retain_exact_carriers() {
    let source = "SELECT Id,IIf(Id=1,CDec('0.1'),IIf(Id=2,CDbl(0.1),CCur('0.1'))) AS K \
                  FROM (SELECT 1 AS Id UNION ALL SELECT 2 UNION ALL SELECT 3) X";
    let groups =
        format!("SELECT VarType(K) AS T,Count(*) AS N FROM ({source}) Q GROUP BY K ORDER BY K");
    let integers = "SELECT TOP 1 K FROM \
                    (SELECT IIf(Id=1,CLngLng('9007199254740993'),CDbl(9007199254740992)) AS K \
                    FROM (SELECT 1 AS Id UNION ALL SELECT 2) X) Q ORDER BY K DESC";

    for declared in [false, true] {
        for (sql, expected) in [
            (groups.as_str(), b"T\tN\n14\t2\n5\t1\n".as_slice()),
            (integers, b"K\n9007199254740993\n".as_slice()),
        ] {
            let mut arguments = vec!["query", "run", sql, "--number-format", "invariant"];
            if declared {
                arguments.push("--declared-arithmetic");
            }

            let result = run(&arguments);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout, expected);
        }
    }
}

#[test]
fn failed_later_operands_do_not_publish_an_ordered_prefix() {
    let result = run(&[
        "query",
        "run",
        "SELECT DISTINCT IIf(Id=1,CDec('0.1'),CDbl(1)/0) AS K \
         FROM (SELECT 1 AS Id UNION ALL SELECT 2) X ORDER BY K",
        "--number-format",
        "invariant",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("division by zero"));
}
