mod support;
use support::run;

#[test]
fn math_numeric_text_uses_explicit_format_and_buffers_failures() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Abs('-2,5') AS A,RGB('2,5','1,5','3,5') AS R",
            "--number-format",
            "de-de",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"A\tR\n2.5\t262658\n");
        let failure = run(&["query", mode, "SELECT 1,Abs('-2,5')"]);
        assert_eq!(failure.status.code(), Some(2));
        assert!(failure.stdout.is_empty());
        assert!(String::from_utf8_lossy(&failure.stderr).contains("number-format"));
    }
}
