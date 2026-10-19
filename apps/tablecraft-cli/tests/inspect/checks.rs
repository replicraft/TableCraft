use super::{Fixture, run};

fn bytes() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}
#[test]
fn completed_check_reports_partial_coverage_and_preserves_original_bytes() {
    let original = bytes();
    let fixture = Fixture::new(&original);
    let output = run(&[
        "inspect",
        fixture.path(),
        "--table",
        "ROULETTE_TB",
        "--check",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"Table\tROULETTE_TB\nRows checked\t38\nIndexes checked\t2\nIndexes not checked\t1\nIndex\tStatus\tIndex constraints\nCOLOR_ID\tChecked\tChecked: exact unique/required\nNumber\tNot checked: unsupported profile\tNot checked\nORDR_NBR\tChecked\tChecked: exact unique/required\n");
    assert!(output.stderr.is_empty());
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
    for args in [
        vec!["inspect", fixture.path(), "--check"],
        vec![
            "inspect",
            fixture.path(),
            "--table",
            "ROULETTE_TB",
            "--check",
            "--indexes",
        ],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}
#[test]
fn failed_check_has_no_partial_stdout_and_diagnostics_omit_payloads() {
    let mut corrupt = bytes();
    corrupt[731 * 4096] = 0;
    let fixture = Fixture::new(&corrupt);
    let output = run(&[
        "-vv",
        "inspect",
        fixture.path(),
        "--table",
        "ROULETTE_TB",
        "--check",
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let logs = String::from_utf8(output.stderr).unwrap();
    assert!(logs.contains("native.table.check"));
    for payload in [
        fixture.path(),
        "ROULETTE_TB",
        "ORDR_NBR",
        "COLOR_ID",
        "NBR_VALUE",
    ] {
        assert!(!logs.contains(payload));
    }
}
