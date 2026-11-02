use tablecraft_testkit::support::cli::Runner;

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

#[test]
fn unary_string_error_data_and_late_operational_failures_stay_buffered() {
    for mode in ["eval", "run"] {
        let result = CLI.run(&[
            "query",
            mode,
            "SELECT Len(CStr(CVErr(11))) AS N,AscW(CStr(CVErr(11))) AS C,Val(CStr(CVErr(11))) AS V",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"N\tC\tV\n8\t69\t0\n");

        for function in [
            "Len",
            "LTrim",
            "RTrim",
            "Trim",
            "LCase",
            "UCase",
            "Asc",
            "AscW",
            "StrReverse",
            "Val",
        ] {
            for (operand, diagnostic) in [
                ("CVErr(11)", "type mismatch"),
                ("IIf(True,CVErr(11),1/0)", "division by zero"),
            ] {
                let sql = format!("SELECT 1,{function}({operand})");
                let result = CLI.run(&["query", mode, &sql]);
                assert!(!result.status.success(), "{sql}");
                assert!(result.stdout.is_empty(), "{sql}");
                assert!(
                    String::from_utf8_lossy(&result.stderr).contains(diagnostic),
                    "{sql}"
                );
            }
        }
    }
}
