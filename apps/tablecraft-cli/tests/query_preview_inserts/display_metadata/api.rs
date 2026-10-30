use super::*;

#[test]
fn sql_incrementing_insert_preserves_existing_display_format_and_exact_whole_image() {
    let (before, wanted) = display_case();
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.accdb");
    let output = directory.path().join("candidate.accdb");
    fs::write(&input, &before).unwrap();
    let result = run("INSERT INTO Items (Id) VALUES (4)", &input, &output, true);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    let report = String::from_utf8(result.stdout).unwrap();
    assert!(report.contains("Generated column\t1\n"));
    assert!(report.contains("Generated value[0]\t31\n"));
    let actual = fs::read(&output).unwrap();
    assert_eq!(actual.len(), wanted.len());
    assert_eq!(
        actual
            .iter()
            .zip(&wanted)
            .position(|(actual, wanted)| actual != wanted),
        None
    );
    assert_eq!(fs::read(&input).unwrap(), before);
}
