//! Executable DELETE workflow over actual repository-authored fixture owners.
#[path = "../../../crates/engines/tablecraft-ace/tests/support/agile.rs"]
mod encrypted_fixture;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

use std::{
    fs,
    io::Cursor,
    path::Path,
    process::{Command, Output},
};
use tablecraft_ace::{
    EncryptedSource, IndexCheckStatus, RuleCheckStatus, check_table, check_table_constraints,
    read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn source(code: u8) -> Vec<u8> {
    let mut before = fixture::database();
    before[4..20].copy_from_slice(b"Standard ACE DB\0");
    before[20] = code;
    before
}

fn expected(before: &[u8], selected: &[usize]) -> Vec<u8> {
    let mut after = before.to_vec();
    after[10 * 4096 + 16..10 * 4096 + 20]
        .copy_from_slice(&(3_u32 - selected.len() as u32).to_le_bytes());

    for &row in selected {
        let slot = 12 * 4096 + 14 + row * 2;
        let original = u16::from_le_bytes(before[slot..slot + 2].try_into().unwrap());
        after[slot..slot + 2].copy_from_slice(&(original | 0xc000).to_le_bytes());
    }

    after
}

fn rows(bytes: &[u8]) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .into_parts()
    .2
}

fn qualify(bytes: &[u8], expected_rows: usize) {
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), expected_rows);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );

    let report = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(report.table_validation(), RuleCheckStatus::Unsupported);
    assert!(report.relationships().is_empty());
    assert!(report.fields().iter().all(|field| {
        field.required() != RuleCheckStatus::Unsupported
            && field.empty_text() != RuleCheckStatus::Unsupported
            && field.validation() != RuleCheckStatus::Unsupported
    }));
}

fn run(sql: &str, input: &Path, output: &Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "preview-delete", sql, "--database"])
        .arg(input)
        .arg("--output")
        .arg(output)
        .args(flags)
        .output()
        .unwrap()
}

fn success(result: Output, matched: usize, deleted: usize) {
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());

    let report = String::from_utf8(result.stdout).unwrap();
    assert!(report.contains("Experimental"));
    assert!(report.contains(&format!("Matched rows\t{matched}\n")));
    assert!(report.contains(&format!("Deleted rows\t{deleted}\n")));
}

#[test]
fn qualify_executable_sql_delete_sources_and_full_expected_before_actions() {
    for code in [5, 6] {
        let before = source(code);
        qualify(&before, 3);

        for selected in [vec![], vec![1], vec![0, 1, 2]] {
            let after = expected(&before, &selected);
            qualify(&after, 3 - selected.len());
            let retained = (0..3)
                .filter(|row| !selected.contains(row))
                .map(|row| {
                    vec![
                        Value::Int32(row as i32 + 1),
                        Value::Int32((row as i32 + 1) * 10),
                    ]
                })
                .collect::<Vec<_>>();
            assert_eq!(rows(&after), retained);
        }
    }

    let plain = source(5);
    let encrypted = encrypted_fixture::encrypted_image(&plain, 5, "public test password");
    let mut decoded = EncryptedSource::open(
        Cursor::new(encrypted),
        Some("public test password"),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        read_table(
            &mut decoded,
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .rows(),
        rows(&plain)
    );
}

#[test]
fn executable_selected_noop_and_all_deletes_preserve_exact_source_bytes() {
    for code in [5, 6] {
        let before = source(code);
        qualify(&before, 3);
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.accdb");
        fs::write(&input, &before).unwrap();

        for (name, sql, selected, flags) in [
            (
                "selected.accdb",
                "DELETE i.* FROM Items AS i WHERE i.Id=2",
                vec![1],
                vec![],
            ),
            (
                "noop.accdb",
                "DELETE FROM Items WHERE FALSE",
                vec![],
                vec![],
            ),
            (
                "all.accdb",
                "DELETE * FROM Items",
                vec![0, 1, 2],
                vec!["--cascade-deletes"],
            ),
        ] {
            let after = expected(&before, &selected);
            qualify(&after, 3 - selected.len());
            let output = directory.path().join(name);

            success(
                run(sql, &input, &output, &flags),
                selected.len(),
                selected.len(),
            );
            assert_eq!(fs::read(&output).unwrap(), after);
            assert_eq!(fs::read(&input).unwrap(), before);
            qualify(&fs::read(&output).unwrap(), 3 - selected.len());
        }
    }
}

#[test]
fn executable_refusals_leave_sources_destinations_and_directory_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.accdb");
    let output = directory.path().join("refused.accdb");

    for sql in ["DELETE wrong.* FROM Items", "UPDATE Items SET Score=1"] {
        let result = run(sql, &input, &output, &[]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("NotFound"));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    let before = source(5);
    qualify(&before, 3);
    fs::write(&input, &before).unwrap();

    let result = run("DELETE FROM Items WHERE 10/(2-Id)>0", &input, &output, &[]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(!output.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    assert_eq!(fs::read(&input).unwrap(), before);

    fs::write(&output, b"existing destination").unwrap();
    let result = run("DELETE FROM Items WHERE Id=2", &input, &output, &[]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert_eq!(fs::read(&output).unwrap(), b"existing destination");

    let result = run("DELETE FROM Items WHERE Id=2", &input, &input, &[]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert_eq!(fs::read(&input).unwrap(), before);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn executable_encrypted_source_refuses_even_with_the_correct_password() {
    let before = source(5);
    let ciphertext = encrypted_fixture::encrypted_image(&before, 5, "public test password");
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("encrypted.accdb");
    let output = directory.path().join("plaintext.accdb");
    fs::write(&input, &ciphertext).unwrap();

    for flags in [vec![], vec!["--password", "public test password"]] {
        let result = run("DELETE FROM Items WHERE Id=2", &input, &output, &flags);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("encrypted rewrite"));
        assert!(!output.exists());
        assert_eq!(fs::read(&input).unwrap(), ciphertext);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

#[test]
fn deterministic_function_predicates_publish_the_original_exact_delete_images() {
    let directory = tempfile::tempdir().unwrap();
    for code in [5, 6] {
        for (position, (predicate, selected)) in [
            ("Len(Trim('  x  '))=0", vec![]),
            ("Abs(Id-2)=0", vec![1]),
            ("CLng(Round(Id/2.0,0))<>1", vec![0, 2]),
            ("Nz(Id,0)=Id", vec![0, 1, 2]),
        ]
        .iter()
        .enumerate()
        {
            let before = source(code);
            let after = expected(&before, selected);
            let input = directory
                .path()
                .join(format!("{code}-{position}-functions.accdb"));
            let output = directory
                .path()
                .join(format!("{code}-{position}-deleted.accdb"));
            fs::write(&input, &before).unwrap();

            success(
                run(
                    &format!("DELETE FROM Items WHERE {predicate}"),
                    &input,
                    &output,
                    &[],
                ),
                selected.len(),
                selected.len(),
            );
            assert_eq!(fs::read(&output).unwrap(), after);
            assert_eq!(fs::read(&input).unwrap(), before);
        }
    }
}

#[test]
fn eager_and_forbidden_function_predicates_leave_no_output() {
    let directory = tempfile::tempdir().unwrap();
    let before = source(5);
    let input = directory.path().join("functions-source.accdb");
    fs::write(&input, &before).unwrap();

    for (position, predicate) in [
        "IIf(Id>0,TRUE,1/0)",
        "Nz(Id,1/0)=Id",
        "FALSE AND Rnd()>0",
        "FALSE AND DCount('*','Hidden')>0",
    ]
    .iter()
    .enumerate()
    {
        let output = directory
            .path()
            .join(format!("function-refused-{position}.accdb"));
        let result = run(
            &format!("DELETE FROM Items WHERE {predicate}"),
            &input,
            &output,
            &[],
        );
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
        assert_eq!(fs::read(&input).unwrap(), before);
    }
}
