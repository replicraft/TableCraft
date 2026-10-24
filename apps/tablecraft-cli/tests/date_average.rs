mod support;

use support::run;

#[test]
fn date_average_cli_projects_serial_means_and_retains_atomic_failures() {
    let result = run(&[
        "query",
        "run",
        "SELECT Avg(Moment) AS M,TypeName(Avg(Moment)) AS Kind FROM (SELECT #2024-01-01# AS Moment UNION ALL SELECT NULL UNION ALL SELECT #2024-01-03#) AS T",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let records = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(result.stdout.as_slice())
        .records()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].iter().collect::<Vec<_>>(),
        ["45293", "\"Double\""]
    );

    let result = run(&[
        "query",
        "run",
        "SELECT 1,Avg(#2024-01-01#+1/(N-2)) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS T",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}
