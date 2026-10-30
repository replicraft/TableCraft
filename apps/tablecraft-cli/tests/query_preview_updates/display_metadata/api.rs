use super::*;

#[test]
fn sql_text_update_and_delete_preserve_format_and_prefrozen_complete_images() {
    let (before, updated, deleted) = display_case();
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.accdb");
    fs::write(&input, &before).unwrap();
    let output = directory.path().join("updated.accdb");
    success(
        run(
            "UPDATE Items SET Label='m' WHERE Id=1",
            &input,
            &output,
            &["--variable"],
        ),
        1,
        1,
    );
    let actual = fs::read(&output).unwrap();
    assert_eq!(actual.len(), updated.len());
    assert_eq!(
        actual
            .iter()
            .zip(&updated)
            .position(|(actual, wanted)| actual != wanted),
        None
    );

    let output = directory.path().join("deleted.accdb");
    let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args([
            "query",
            "preview-delete",
            "DELETE FROM Items WHERE Id=2",
            "--database",
        ])
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    let report = String::from_utf8(result.stdout).unwrap();
    assert!(report.contains("Matched rows\t1\n"));
    assert!(report.contains("Deleted rows\t1\n"));
    let actual = fs::read(&output).unwrap();
    assert_eq!(actual.len(), deleted.len());
    assert_eq!(
        actual
            .iter()
            .zip(&deleted)
            .position(|(actual, wanted)| actual != wanted),
        None
    );
    assert_eq!(fs::read(&input).unwrap(), before);
}
