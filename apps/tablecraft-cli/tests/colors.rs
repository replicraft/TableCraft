mod support;

use support::run;

#[test]
fn rgb_checks_integer_channels_before_saturation_and_keeps_stdout_buffered() {
    for mode in ["eval", "run"] {
        let result = run(&["query", mode, "SELECT RGB(32767,32767,32767) AS C"]);
        assert!(result.status.success());
        assert_eq!(result.stdout, b"C\n16777215\n");

        for (expression, diagnostic) in [
            ("RGB(32768,0,0)", "numeric overflow"),
            ("RGB(0,-32769,0)", "numeric overflow"),
            ("RGB(0,0,32767.5)", "numeric overflow"),
            ("RGB('32768',0,0)", "numeric overflow"),
            ("RGB(-32768,0,0)", "invalid arithmetic domain"),
            ("RGB(32768,1/0,0)", "division by zero"),
        ] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT 1,{expression}"),
                "--number-format",
                "invariant",
            ]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains(diagnostic));
        }
    }
}

#[test]
fn packed_color_queries_work_in_both_paths_without_runtime_context() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT RGB(1,2,3) AS Packed,QBColor(4) AS DarkRed,RGB(999,0,0) AS Clamped",
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
                vec!["Packed", "DarkRed", "Clamped"],
                vec!["197121", "128", "255"]
            ]
        );

        for expression in [
            "RGB(Null,1/0,0)",
            "RGB(1,2)",
            "RGB(1,2,3,4)",
            "RGB(-1,0,0)",
            "RGB('1',2,3)",
            "QBColor(16)",
            "QBColor(*)",
            "VBA.RGB(1,2,3)",
            "Round(1,2,3)",
            "IIf(False,QBColor(16),0)",
            "RGB(1,2,3),1/0",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn palette_integer_conversion_errors_remain_buffered_and_differ_from_domain_failures() {
    for mode in ["eval", "run"] {
        for (input, diagnostic) in [
            ("32768", "numeric overflow"),
            ("-32769", "numeric overflow"),
            ("32767.5", "numeric overflow"),
            ("'32768'", "numeric overflow"),
            ("32767", "invalid arithmetic domain"),
            ("-32768", "invalid arithmetic domain"),
        ] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT 1,QBColor({input})"),
                "--number-format",
                "invariant",
            ]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains(diagnostic));
        }
    }
}
