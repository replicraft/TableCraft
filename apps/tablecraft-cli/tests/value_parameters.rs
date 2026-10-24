mod support;

use support::run;

#[test]
fn value_parameters_keep_explicit_cli_carriers_and_variant_arithmetic() {
    for (parameter, expected) in [
        ("P=short:7", "7"),
        ("P=long:2147483647", "2147483647"),
        ("P=decimal:1.2300", "1.2300"),
        ("P=text:SELECT 1;", "\"SELECT 1;\""),
        ("P=null:", "NULL"),
    ] {
        let result = run(&[
            "query",
            "run",
            "PARAMETERS P VALUE; SELECT P AS V",
            "--param",
            parameter,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice());
        let rows = reader.records().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(&rows[0][0], expected);
    }

    let result = run(&[
        "query",
        "run",
        "PARAMETERS P VALUE; SELECT P+CInt(1) AS N",
        "--param",
        "P=short:32767",
        "--declared-arithmetic",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"N\n32768\n");
}

#[test]
fn value_parameter_failures_never_publish_an_earlier_result() {
    for arguments in [
        vec![],
        vec!["--param", "P=double:NaN"],
        vec!["--param", "P=long:1", "--param", "p=long:2"],
        vec!["--param", "P=long:3"],
    ] {
        let mut command = vec![
            "query",
            "run",
            "PARAMETERS P VALUE; SELECT CInt(1) AS N UNION ALL SELECT CInt(1)/(P-CInt(3))",
        ];
        command.extend(arguments);
        let result = run(&command);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}
