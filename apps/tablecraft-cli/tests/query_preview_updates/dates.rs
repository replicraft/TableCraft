//! Actual date-expression publication and atomic refusal through the CLI.
use super::*;

#[test]
fn deterministic_date_components_publish_and_errors_leave_no_copy() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.accdb");
    let output = directory.path().join("dated.accdb");
    let before = source::source();
    fs::write(&input, &before).unwrap();

    success(
        run(
            "UPDATE Items SET Score=Year(DateSerial(2000+Id,1,1)) WHERE Id=3",
            &input,
            &output,
            &[],
        ),
        1,
        1,
    );
    let after = fs::read(&output).unwrap();
    let mut expected = rows(&before);
    expected[2][2] = Value::Int32(2003);
    assert_eq!(rows(&after), expected);
    assert_eq!(after.len(), before.len());
    assert_eq!(fs::read(&input).unwrap(), before);

    for (index, sql) in [
        "UPDATE Items SET Score=DateSerial(2024,1,1)",
        "UPDATE Items SET Score=Year(DateSerial(2024,Id,1/0))",
        "UPDATE Items SET Score=Year(Now()) WHERE FALSE",
    ]
    .into_iter()
    .enumerate()
    {
        let refused = directory.path().join(format!("refused-{index}.accdb"));
        let result = run(sql, &input, &refused, &[]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!refused.exists());
        assert_eq!(fs::read(&input).unwrap(), before);
    }
}
