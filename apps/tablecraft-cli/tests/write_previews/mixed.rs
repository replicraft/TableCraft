//! Typed write commands reuse the actual qualified mixed source/expected owner.
use super::{fs, growing::source_model, model, qualify};
use std::{
    path::Path,
    process::{Command, Output},
};

#[test]
fn file_transport_preserves_qualified_joint_text_and_binary_requests() {
    for family in [model::Family::Legacy, model::Family::Binary] {
        let case = source_model::case(6, family, false, source_model::Destination::EndOfFile);
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("before.accdb");
        let output = directory.path().join("after.accdb");
        let requests = directory.path().join("requests.tsv");
        fs::write(&input, &case.before).unwrap();
        let binary = matches!(family, model::Family::Binary);
        fs::write(
            &requests,
            format!(
                "Items\tAge\t3\tint32\tnull\t40\nItems\tName\t3\t{}\tnull\t{}\n",
                if binary { "binary" } else { "text" },
                if binary { "00ff" } else { "aa" },
            ),
        )
        .unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["write", "preview-update"])
            .arg(&input)
            .arg(&output)
            .arg("--set-file")
            .arg(&requests)
            .arg("--variable")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let actual = fs::read(&output).unwrap();
        assert!(actual == case.expected, "qualified joint complete image");
        assert!(fs::read(&input).unwrap() == case.before);
    }
}

fn run(
    before: &Path,
    after: &Path,
    binary: bool,
    age_expected: &str,
    name_expected: &str,
    ordinal: &str,
    flags: &[&str],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
    command
        .args(["write", "preview-update"])
        .arg(before)
        .arg(after)
        .args([
            "--set",
            "Items",
            "Age",
            ordinal,
            "int32",
            age_expected,
            "40",
            "--set",
            "Items",
            "Name",
            ordinal,
            if binary { "binary" } else { "text" },
            name_expected,
            if binary { "00ff" } else { "aa" },
        ]);
    command.args(flags).output().unwrap()
}

#[test]
fn qualify_typed_write_mixed_sources_before_actions() {
    for code in [5, 6] {
        for family in [
            model::Family::Legacy,
            model::Family::Modern,
            model::Family::Binary,
        ] {
            for descending in [false, true] {
                for destination in [
                    source_model::Destination::OriginalPage,
                    source_model::Destination::OtherOwned,
                    source_model::Destination::EndOfFile,
                ] {
                    let case = source_model::case(code, family, descending, destination);
                    qualify(&case.before, &case.rows, case.family);
                    qualify(&case.expected, &case.final_rows, case.family);
                    let at = 12 * model::PAGE + 14 + 3 * 2;
                    let word = u16::from_le_bytes(case.expected[at..at + 2].try_into().unwrap());
                    let start = 12 * model::PAGE + usize::from(word & 0x1fff);
                    assert_eq!(&case.expected[start..start + 4], &case.target.to_le_bytes());
                }
            }
        }
    }
}

#[test]
fn typed_mixed_expected_values_publish_all_36_exact_images_including_binary_hex() {
    for code in [5, 6] {
        for family in [
            model::Family::Legacy,
            model::Family::Modern,
            model::Family::Binary,
        ] {
            for descending in [false, true] {
                for destination in [
                    source_model::Destination::OriginalPage,
                    source_model::Destination::OtherOwned,
                    source_model::Destination::EndOfFile,
                ] {
                    let case = source_model::case(code, family, descending, destination);
                    let directory = tempfile::tempdir().unwrap();
                    let before = directory.path().join("before.accdb");
                    let after = directory.path().join("after.accdb");
                    fs::write(&before, &case.before).unwrap();
                    let result = run(
                        &before,
                        &after,
                        matches!(family, model::Family::Binary),
                        "null",
                        "null",
                        "3",
                        &["--variable"],
                    );
                    assert!(
                        result.status.success(),
                        "{}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                    assert!(result.stderr.is_empty());
                    assert_eq!(fs::read(&after).unwrap(), case.expected);
                    assert_eq!(fs::read(&before).unwrap(), case.before);
                }
            }
        }
    }
}

#[test]
fn typed_mixed_stale_cells_ordinals_and_existing_outputs_preserve_all_files() {
    for binary in [false, true] {
        let family = if binary {
            model::Family::Binary
        } else {
            model::Family::Legacy
        };
        let case = source_model::case(6, family, true, source_model::Destination::EndOfFile);
        let directory = tempfile::tempdir().unwrap();
        let before = directory.path().join("before.accdb");
        fs::write(&before, &case.before).unwrap();
        for (ordinal, age, name) in [
            ("3", "0", "null"),
            ("3", "null", if binary { "ff" } else { "value:null" }),
            ("4", "null", "null"),
        ] {
            let after = directory.path().join("refused.accdb");
            let result = run(&before, &after, binary, age, name, ordinal, &["--variable"]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(!after.exists());
            assert_eq!(fs::read(&before).unwrap(), case.before);
        }
        let after = directory.path().join("existing.accdb");
        fs::write(&after, b"existing destination").unwrap();
        let result = run(
            &before,
            &after,
            binary,
            "null",
            "null",
            "3",
            &["--variable"],
        );
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert_eq!(fs::read(&after).unwrap(), b"existing destination");
        assert_eq!(fs::read(&before).unwrap(), case.before);
    }
}

#[test]
fn typed_mixed_profile_refusals_happen_before_database_open() {
    let directory = tempfile::tempdir().unwrap();
    let before = directory.path().join("missing.accdb");
    let after = directory.path().join("unpublished.accdb");
    for (table, first_kind, second_kind, flags) in [
        ("OtherTable", "int32", "text", &["--variable"][..]),
        (
            "OtherTable",
            "int32",
            "text",
            &["--variable", "--index-growth"][..],
        ),
        ("Items", "int32", "text", &["--long-value"][..]),
        ("Items", "int32", "unknown-kind", &["--variable"][..]),
        ("Items", "int32", "int32", &["--variable"][..]),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["write", "preview-update"])
            .arg(&before)
            .arg(&after)
            .args([
                "--set",
                "Items",
                "Age",
                "3",
                first_kind,
                "null",
                "null",
                "--set",
                table,
                "Name",
                "3",
                second_kind,
                "null",
                "null",
            ])
            .args(flags)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("NotFound"));
        assert!(!after.exists());
    }

    // Two variable scopes are valid protocol. Native field kinds still need
    // the authorized source, so this case must reach the missing-file open.
    let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["write", "preview-update"])
        .arg(&before)
        .arg(&after)
        .args([
            "--variable",
            "--set",
            "Items",
            "Age",
            "3",
            "text",
            "null",
            "null",
            "--set",
            "Items",
            "Name",
            "3",
            "text",
            "null",
            "null",
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("NotFound"));
    assert!(!after.exists());
}
