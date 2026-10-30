//! Required captured-parent publication of the unchanged qualified index images.
#[cfg(unix)]
use super::{model, source};
use std::{fs, process::Command};

#[cfg(unix)]
#[test]
fn required_index_publication_matches_all_sixteen_whole_expected_images() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for code in [5, 6] {
        for population in model::POPULATIONS {
            for descending in [false, true] {
                let case = model::source(code, population, descending);
                let input = directory.path().join(format!("source-{count}.accdb"));
                let output = directory.path().join(format!("copy-{count}.accdb"));
                fs::write(&input, &case.before).unwrap();
                let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
                command
                    .args(["query", "preview-create-index", &source::sql(&case)])
                    .arg("--database")
                    .arg(&input)
                    .arg("--output")
                    .arg(&output)
                    .arg("--durable-output");
                let result = command.output().unwrap();
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                assert!(result.stderr.is_empty());
                let report = format!(
                    "Engineering copy\tExperimental; data macros and native Access saves unqualified\nCreated index\t{}\nLogical index number\t4\nPhysical index number\t2\nIndexed source rows\t{}\nDurable output\tFile and captured-directory barriers completed\n",
                    case.name,
                    case.rows.len(),
                );
                assert_eq!(result.stdout, report.as_bytes());
                source::same_bytes(&fs::read(&output).unwrap(), &case.expected);
                source::same_bytes(&fs::read(&input).unwrap(), &case.before);

                let occupied = command.output().unwrap();
                assert_eq!(occupied.status.code(), Some(2));
                assert!(occupied.stdout.is_empty());
                assert!(
                    String::from_utf8_lossy(&occupied.stderr).contains("output was not published")
                );
                source::same_bytes(&fs::read(&output).unwrap(), &case.expected);
                source::same_bytes(&fs::read(&input).unwrap(), &case.before);
                count += 1;
            }
        }
    }
    assert_eq!(count, 16);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 32);
}

#[cfg(not(unix))]
#[test]
fn unsupported_required_index_output_refuses_before_source_open() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("unopened.accdb");
    let output = directory.path().join("copy.accdb");
    let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args([
            "query",
            "preview-create-index",
            "CREATE INDEX LookupScore ON Items (Score)",
            "--database",
        ])
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .arg("--durable-output")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("durability is unsupported"));
    assert!(!output.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}
