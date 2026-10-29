//! Actual user-facing engineering-copy workflows over qualified literal sources.
#[path = "../../../crates/engines/tablecraft-ace/tests/support/agile.rs"]
mod encrypted_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/index_fixture.rs"]
mod index_fixture;
#[path = "write_previews/model.rs"]
mod model;
use tablecraft_testkit::access as support;

use std::{
    fs,
    io::Cursor,
    path::Path,
    process::{Command, Output},
};
use tablecraft_ace::{
    EncryptedSource, IndexCheckStatus, check_table, check_table_constraints, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
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

fn invoke(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(arguments)
        .output()
        .unwrap()
}

fn success(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Experimental"));
    output.stdout
}

fn source(directory: &Path, bytes: &[u8]) -> std::path::PathBuf {
    let path = directory.join("input.accdb");
    fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn qualify_engineering_cli_literal_sources_before_preview_commands() {
    for bytes in [
        model::fixed(false),
        model::fixed(true),
        model::variable(false),
        model::variable(true),
    ] {
        let checked = check_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), 3);
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        let constraints = check_table_constraints(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(constraints.rows(), 3);
        assert_eq!(rows(&bytes).len(), 3);
    }
    let plain = model::fixed(false);
    let ciphertext = encrypted_fixture::encrypted_image(&plain, 5, "public test password");
    assert!(
        tablecraft_ace::inspect_header(&mut Cursor::new(&ciphertext), &mut budget(), &|| false)
            .unwrap()
            .encoded_pages()
    );
    let mut encrypted = EncryptedSource::open(
        Cursor::new(&ciphertext),
        Some("public test password"),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let decoded = read_table(
        &mut encrypted,
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(decoded.rows(), rows(&plain));
}

#[test]
fn fixed_update_insert_generated_insert_and_row_delete_create_readable_copies() {
    let directory = tempfile::tempdir().unwrap();
    let before = model::fixed(false);
    let input = source(directory.path(), &before);
    let update = directory.path().join("updated.accdb");
    success(invoke(&[
        "write",
        "preview-update",
        input.to_str().unwrap(),
        update.to_str().unwrap(),
        "--set",
        "Items",
        "Score",
        "1",
        "int32",
        "20",
        "25",
        "--index-growth",
    ]));
    assert_eq!(rows(&fs::read(&update).unwrap())[1][1], Value::Int32(25));
    let requests = directory.path().join("insert.tsv");
    fs::write(&requests, b"int32\t4\tint32\t40\n").unwrap();
    let inserted = directory.path().join("inserted.accdb");
    success(invoke(&[
        "write",
        "preview-insert",
        input.to_str().unwrap(),
        inserted.to_str().unwrap(),
        "Items",
        requests.to_str().unwrap(),
    ]));
    assert_eq!(
        rows(&fs::read(&inserted).unwrap()),
        vec![
            vec![Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(2), Value::Int32(20)],
            vec![Value::Int32(3), Value::Int32(30)],
            vec![Value::Int32(4), Value::Int32(40)]
        ]
    );
    let deleted = directory.path().join("deleted.accdb");
    success(invoke(&[
        "write",
        "preview-delete",
        input.to_str().unwrap(),
        deleted.to_str().unwrap(),
        "Items",
        "--row",
        "1",
    ]));
    assert_eq!(
        rows(&fs::read(deleted).unwrap()),
        vec![
            vec![Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(3), Value::Int32(30)]
        ]
    );
    assert_eq!(fs::read(&input).unwrap(), before);

    let generated_before = model::fixed(true);
    fs::write(&input, &generated_before).unwrap();
    fs::write(&requests, b"int32\t4\n").unwrap();
    let generated = directory.path().join("generated.accdb");
    let report = success(invoke(&[
        "write",
        "preview-insert",
        input.to_str().unwrap(),
        generated.to_str().unwrap(),
        "Items",
        requests.to_str().unwrap(),
        "--incrementing-last",
        "30",
    ]));
    assert!(String::from_utf8_lossy(&report).contains("Generated column\t1\n"));
    assert!(String::from_utf8_lossy(&report).contains("Generated value[0]\t31\n"));
    assert_eq!(
        rows(&fs::read(generated).unwrap())[3],
        vec![Value::Int32(4), Value::Int32(31)]
    );
    assert_eq!(fs::read(input).unwrap(), generated_before);
}

#[test]
fn ordinary_name_and_binary_edits_keep_null_and_literal_null_distinct() {
    for binary in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let before = model::variable(binary);
        let input = source(directory.path(), &before);
        let output = directory.path().join("updated.accdb");
        let (kind, expected, replacement, value) = if binary {
            ("binary", "00ff", "fe00", Value::Binary(vec![254, 0]))
        } else {
            (
                "text",
                "Alice",
                "value:null",
                Value::Text("null".to_owned()),
            )
        };
        success(invoke(&[
            "write",
            "preview-update",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "--variable",
            "--index-growth",
            "--set",
            "Items",
            "Score",
            "0",
            kind,
            expected,
            replacement,
        ]));
        assert_eq!(rows(&fs::read(output).unwrap())[0][1], value);
        let cleared = directory.path().join("cleared.accdb");
        success(invoke(&[
            "write",
            "preview-update",
            input.to_str().unwrap(),
            cleared.to_str().unwrap(),
            "--variable",
            "--set",
            "Items",
            "Score",
            "0",
            kind,
            expected,
            "null",
        ]));
        assert_eq!(rows(&fs::read(cleared).unwrap())[0][1], Value::Null);
        assert_eq!(fs::read(input).unwrap(), before);
    }
}

#[test]
fn failures_and_normal_save_gate_preserve_source_and_existing_destinations() {
    let directory = tempfile::tempdir().unwrap();
    let before = model::fixed(false);
    let input = source(directory.path(), &before);
    let output = directory.path().join("existing.accdb");
    fs::write(&output, b"existing destination").unwrap();
    let result = invoke(&[
        "write",
        "preview-update",
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        "--set",
        "Items",
        "Score",
        "1",
        "int32",
        "20",
        "25",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert_eq!(fs::read(&output).unwrap(), b"existing destination");
    let refused = directory.path().join("refused.accdb");
    for arguments in [
        vec![
            "write",
            "preview-delete",
            input.to_str().unwrap(),
            refused.to_str().unwrap(),
            "Items",
            "--delete",
            "1:0000",
        ],
        vec![
            "write",
            "preview-delete",
            input.to_str().unwrap(),
            refused.to_str().unwrap(),
            "Items",
            "--row",
            "1",
            "--row",
            "1",
        ],
        vec![
            "query",
            "update",
            "UPDATE Items SET Score=25 WHERE Id=2",
            "--database",
            input.to_str().unwrap(),
            "--output",
            refused.to_str().unwrap(),
        ],
    ] {
        let result = invoke(&arguments);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!refused.exists());
    }
    // Reuse the existing distinct NODE source too: ordinary Jet4 write preview
    // admission must remain refused rather than silently entering an ACE mode.
    fs::write(&input, index_fixture::node_database()).unwrap();
    let result = invoke(&[
        "write",
        "preview-delete",
        input.to_str().unwrap(),
        refused.to_str().unwrap(),
        "Items",
        "--row",
        "0",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(!refused.exists());
    fs::write(&input, &before).unwrap();
    assert_eq!(fs::read(input).unwrap(), before);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn encrypted_source_refuses_before_password_prompt_or_plaintext_publication() {
    let directory = tempfile::tempdir().unwrap();
    let before = model::fixed(false);
    let input = source(directory.path(), &before);
    let encrypted = directory.path().join("encrypted.accdb");
    let supported_cipher = encrypted_fixture::encrypted_image(&before, 5, "public test password");
    fs::write(&encrypted, &supported_cipher).unwrap();
    let captured = fs::read(&encrypted).unwrap();
    let output = directory.path().join("refused.accdb");
    let result = invoke(&[
        "write",
        "preview-delete",
        encrypted.to_str().unwrap(),
        output.to_str().unwrap(),
        "Items",
        "--row",
        "0",
        "--password",
        "public test password",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("encrypted rewrite"));
    assert!(!output.exists());
    assert_eq!(fs::read(encrypted).unwrap(), captured);
    assert_eq!(fs::read(input).unwrap(), before);
}

#[test]
fn explicit_incrementing_uses_the_captured_counter_and_matches_exact_policy_output() {
    let directory = tempfile::tempdir().unwrap();
    let before = model::fixed(true);
    let input = source(directory.path(), &before);
    let requests = directory.path().join("companions.tsv");
    fs::write(&requests, b"int32\t4\nint32\t5\n").unwrap();
    let from_source = directory.path().join("source-counter.accdb");
    let explicit = directory.path().join("explicit-counter.accdb");
    let report = success(invoke(&[
        "write",
        "preview-insert",
        input.to_str().unwrap(),
        from_source.to_str().unwrap(),
        "Items",
        requests.to_str().unwrap(),
        "--incrementing",
    ]));
    assert!(String::from_utf8_lossy(&report).contains("Generated column\t1\n"));
    assert!(String::from_utf8_lossy(&report).contains("Generated value[0]\t31\n"));
    assert!(String::from_utf8_lossy(&report).contains("Generated value[1]\t32\n"));
    success(invoke(&[
        "write",
        "preview-insert",
        input.to_str().unwrap(),
        explicit.to_str().unwrap(),
        "Items",
        requests.to_str().unwrap(),
        "--incrementing-last",
        "30",
    ]));
    let candidate = fs::read(&from_source).unwrap();
    assert_eq!(candidate, fs::read(&explicit).unwrap());
    let actual = rows(&candidate);
    assert_eq!(
        &actual[3..],
        &[
            vec![Value::Int32(4), Value::Int32(31)],
            vec![Value::Int32(5), Value::Int32(32)],
        ]
    );
    assert_eq!(fs::read(&input).unwrap(), before);

    let unchanged = directory.path().join("stale-policy.accdb");
    let refused = invoke(&[
        "write",
        "preview-insert",
        input.to_str().unwrap(),
        unchanged.to_str().unwrap(),
        "Items",
        requests.to_str().unwrap(),
        "--incrementing-last",
        "29",
    ]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(refused.stdout.is_empty());
    assert!(!unchanged.exists());
    assert_eq!(fs::read(input).unwrap(), before);
}
