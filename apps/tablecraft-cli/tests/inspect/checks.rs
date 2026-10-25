use super::{Fixture, run};

fn bytes() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}
#[test]
fn completed_check_reports_qualified_coverage_and_preserves_original_bytes() {
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
    assert_eq!(output.stdout, b"Table\tROULETTE_TB\nRows checked\t38\nIndexes checked\t3\nIndexes not checked\t0\nIndex\tStatus\tIndex constraints\nCOLOR_ID\tChecked\tChecked: exact unique/required\nNumber\tChecked\tChecked: exact unique/required\nORDR_NBR\tChecked\tChecked: exact unique/required\n");
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

#[test]
fn explicit_inspection_work_limits_preserve_independent_byte_limits() {
    let original = bytes();
    let fixture = Fixture::new(&original);
    let base = [
        "inspect",
        fixture.path(),
        "--table",
        "ROULETTE_TB",
        "--check",
    ];
    let reference = run(&base);
    assert!(reference.status.success());

    let mut low = base.to_vec();
    low.extend(["--native-steps", "1"]);
    let refused = run(&low);
    assert_eq!(refused.status.code(), Some(2));
    assert!(refused.stdout.is_empty());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("Steps resource limit"));

    let mut enough = base.to_vec();
    enough.extend(["--native-steps", "10000000"]);
    let completed = run(&enough);
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );
    assert!(completed.stderr.is_empty());
    assert_eq!(completed.stdout, reference.stdout);

    enough.extend(["--native-bytes", "1"]);
    let byte_refusal = run(&enough);
    assert_eq!(byte_refusal.status.code(), Some(2));
    assert!(byte_refusal.stdout.is_empty());
    assert!(String::from_utf8_lossy(&byte_refusal.stderr).contains("Bytes resource limit"));

    for value in ["0", "100000001"] {
        let mut invalid = base.to_vec();
        invalid.extend(["--native-steps", value]);
        let output = run(&invalid);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}
