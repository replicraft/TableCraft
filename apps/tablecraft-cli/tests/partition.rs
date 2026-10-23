mod support;

use support::run;

#[test]
fn padded_partition_labels_use_csv_transport_and_buffer_errors() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Partition(1010,100,1010,20) AS P,Partition(Null,0,9,5) AS N,Partition(100,0,99,5) AS A",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let rows: Vec<Vec<String>> = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_reader(result.stdout.as_slice())
            .records()
            .map(|row| row.unwrap().iter().map(str::to_owned).collect())
            .collect();
        assert_eq!(
            rows,
            [
                vec!["P", "N", "A"],
                vec!["\"1000:1010\"", "NULL", "\"100:   \""]
            ]
        );

        for expression in [
            "Partition(1,0,9)",
            "Partition(*)",
            "Partition(1,,9,5)",
            "Partition(1,0,9,0)",
            "Partition(Null,1/0,9,5)",
            "Partition(1,0,9,5),1/0",
            "VBA.Partition(1,0,9,5)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn partition_numeric_text_controls_respect_format_null_and_buffered_failure() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Partition('2,5','0','9','2') AS P,Partition('bad',NULL,9,2) AS N",
            "--number-format",
            "de-de",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let rows: Vec<Vec<String>> = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_reader(result.stdout.as_slice())
            .records()
            .map(|row| row.unwrap().iter().map(str::to_owned).collect())
            .collect();
        assert_eq!(rows, [vec!["P", "N"], vec!["\" 2: 3\"", "NULL"]]);
        for expression in [
            "Partition('2147483648',0,9,2)",
            "Partition(1,'-1','9','bad')",
        ] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT 1,{expression}"),
                "--number-format",
                "en-us",
            ]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
    }
}
