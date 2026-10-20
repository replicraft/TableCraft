#[path = "../../../crates/engines/tablecraft-ace/tests/support/agile.rs"]
mod fixture;
mod support;
use rc4::{KeyInit, StreamCipher};
use std::{fs, path::Path};
use support::run;

fn plain() -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}

#[test]
fn add_encrypts_each_read_family_and_removal_recovers_all_original_bytes() {
    let directory = tempfile::tempdir().unwrap();
    for code in [1, 2, 3, 5, 6] {
        let mut before = plain();
        before[20] = code;
        if code == 1 {
            before[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        let input = directory.path().join(format!("plain-{code}.db"));
        fs::write(&input, &before).unwrap();
        let mut previous = None;
        for round in 0..2 {
            let encrypted = directory.path().join(format!("added-{code}-{round}.db"));
            let result = run(&[
                "encryption",
                "add",
                input.to_str().unwrap(),
                "--output",
                encrypted.to_str().unwrap(),
                "--new-password",
                "public 😀 password",
            ]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let bytes = fs::read(&encrypted).unwrap();
            assert_ne!(&bytes[4096..], &before[4096..]);
            if let Some(previous) = previous {
                assert_ne!(bytes, previous);
            }
            previous = Some(bytes);
            let query = run(&[
                "query",
                "run",
                "SELECT Count(*) AS Total FROM ROULETTE_TB",
                "--database",
                encrypted.to_str().unwrap(),
                "-p",
                "public 😀 password",
            ]);
            assert!(
                query.status.success(),
                "{}",
                String::from_utf8_lossy(&query.stderr)
            );
            assert_eq!(query.stdout, b"Total\n38\n");
            let refused = directory.path().join("refused.db");
            let result = run(&[
                "encryption",
                "add",
                encrypted.to_str().unwrap(),
                "--output",
                refused.to_str().unwrap(),
                "--new-password",
                "public credential",
            ]);
            assert_eq!(result.status.code(), Some(2));
            assert!(!refused.exists());
            let result = run(&[
                "encryption",
                "remove",
                encrypted.to_str().unwrap(),
                "--output",
                refused.to_str().unwrap(),
                "-p",
                "wrong",
            ]);
            assert_eq!(result.status.code(), Some(2));
            assert!(!refused.exists());
            let removed = directory.path().join(format!("removed-{code}-{round}.db"));
            let result = run(&[
                "encryption",
                "remove",
                encrypted.to_str().unwrap(),
                "--output",
                removed.to_str().unwrap(),
                "-p",
                "public 😀 password",
            ]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let mut expected = before.clone();
            if code == 2 {
                expected[20..22].copy_from_slice(&[3, 1]);
            }
            assert_eq!(fs::read(removed).unwrap(), expected);
        }
        assert_eq!(fs::read(input).unwrap(), before);
    }
}

#[test]
fn invalid_add_credentials_and_existing_outputs_never_publish_a_copy() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("plain.accdb");
    let bytes = plain();
    fs::write(&input, &bytes).unwrap();
    let output = directory.path().join("output.accdb");
    let too_long = "a".repeat(256);
    for args in [
        vec![],
        vec!["--new-password", ""],
        vec!["--new-password", too_long.as_str()],
    ] {
        let mut command = vec![
            "encryption",
            "add",
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ];
        command.extend(args);
        let result = run(&command);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
    }
    fs::write(&output, b"keep existing output").unwrap();
    let result = run(&[
        "encryption",
        "add",
        input.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
        "--new-password",
        "public credential",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(fs::read(&output).unwrap(), b"keep existing output");
    assert_eq!(fs::read(input).unwrap(), bytes);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}
fn jet_image() -> Vec<u8> {
    let mut bytes = plain();
    bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    bytes[20] = 1;
    let key = [0x55u8, 0x66, 0x77, 0x88];
    for (b, key) in bytes[62..66].iter_mut().zip(key) {
        *b ^= key;
    }
    for (number, page) in bytes
        .as_chunks_mut::<4096>()
        .0
        .iter_mut()
        .enumerate()
        .skip(1)
    {
        let mut page_key = (number as u32).to_le_bytes();
        for (b, key) in page_key.iter_mut().zip(key) {
            *b ^= key;
        }
        rc4::Rc4::new_from_slice(&page_key)
            .unwrap()
            .apply_keystream(page);
    }
    bytes
}

#[test]
fn agile_rewrites_reopen_all_read_profiles_preserving_pages_and_sources() {
    let plain = plain();
    let directory = tempfile::tempdir().unwrap();
    for code in [2, 3, 5, 6] {
        let encrypted = fixture::encrypted_image(&plain, code, "public 😀 password");
        let input = directory.path().join(format!("source-{code}.accdb"));
        fs::write(&input, &encrypted).unwrap();
        let input = input.to_str().unwrap();
        let mut first_changed = None;
        for round in 0..2 {
            let output = directory
                .path()
                .join(format!("changed-{code}-{round}.accdb"));
            let result = run(&[
                "encryption",
                "change-password",
                input,
                "--output",
                output.to_str().unwrap(),
                "-p",
                "public 😀 password",
                "--new-password",
                "new public 😀 credential",
            ]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let after = fs::read(&output).unwrap();
            assert_eq!(&after[4096..], &encrypted[4096..]);
            assert_eq!(&after[..152], &encrypted[..152]);
            if let Some(first) = first_changed {
                assert_ne!(first, after, "fresh salts/verifiers required");
            }
            first_changed = Some(after);
            let query = run(&[
                "query",
                "run",
                "SELECT Count(*) AS Total FROM ROULETTE_TB",
                "--database",
                output.to_str().unwrap(),
                "-p",
                "new public 😀 credential",
            ]);
            assert!(
                query.status.success(),
                "{}",
                String::from_utf8_lossy(&query.stderr)
            );
            assert_eq!(query.stdout, b"Total\n38\n");
            let old = run(&[
                "tables",
                output.to_str().unwrap(),
                "-p",
                "public 😀 password",
            ]);
            assert_eq!(old.status.code(), Some(2));
            assert!(old.stdout.is_empty());
        }
        let output = directory.path().join(format!("removed-{code}.accdb"));
        let result = run(&[
            "encryption",
            "remove",
            input,
            "--output",
            output.to_str().unwrap(),
            "-p",
            "public 😀 password",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let after = fs::read(&output).unwrap();
        assert_eq!(after[20..24], encrypted[20..24]); // retain compatibility subversion one
        assert_eq!(&after[4096..], &plain[4096..]);
        let query = run(&[
            "query",
            "run",
            "SELECT Count(*) AS Total FROM ROULETTE_TB",
            "--database",
            output.to_str().unwrap(),
        ]);
        assert!(
            query.status.success(),
            "{}",
            String::from_utf8_lossy(&query.stderr)
        );
        assert_eq!(query.stdout, b"Total\n38\n");
        assert_eq!(fs::read(input).unwrap(), encrypted);
    }
}

#[test]
fn jet_encoding_and_password_changes_round_trip_without_losing_native_records() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.mdb");
    let original = jet_image();
    fs::write(&input, &original).unwrap();
    let changed = directory.path().join("changed.mdb");
    let result = run(&[
        "encryption",
        "change-password",
        input.to_str().unwrap(),
        "--output",
        changed.to_str().unwrap(),
        "--new-password",
        "public password",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let removed = directory.path().join("removed.mdb");
    let wrong = run(&[
        "encryption",
        "remove",
        changed.to_str().unwrap(),
        "--output",
        removed.to_str().unwrap(),
        "-p",
        "wrong",
    ]);
    assert_eq!(wrong.status.code(), Some(2));
    assert!(!removed.exists());
    let result = run(&[
        "encryption",
        "remove",
        changed.to_str().unwrap(),
        "--output",
        removed.to_str().unwrap(),
        "-p",
        "public password",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut expected = plain();
    expected[4..20].copy_from_slice(b"Standard Jet DB\0");
    expected[20] = 1;
    assert_eq!(fs::read(&removed).unwrap(), expected);
    let result = run(&["tables", removed.to_str().unwrap(), "--count"]);
    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("ROULETTE_TB\tUser\t38"));
    assert_eq!(fs::read(input).unwrap(), original);
}

#[test]
fn failed_rewrites_never_publish_outputs_overwrite_files_or_expose_credentials() {
    let directory = tempfile::tempdir().unwrap();
    let original = fixture::encrypted_image(&plain(), 5, "public secret");
    let input = directory.path().join("private-source.accdb");
    fs::write(&input, &original).unwrap();
    let output = directory.path().join("output.accdb");
    for extra in [
        vec![],
        vec!["-p", "private-wrong-secret"],
        vec!["-p", "public secret", "--new-password", ""],
    ] {
        let action = if extra.contains(&"--new-password") {
            "change-password"
        } else {
            "remove"
        };
        let mut args = vec![
            "-vvv",
            "encryption",
            action,
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ];
        args.extend(extra);
        let result = run(&args);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
        let text = String::from_utf8_lossy(&result.stderr);
        assert!(!text.contains("private-wrong-secret"));
        assert!(!text.contains("private-source"));
    }
    fs::write(&output, b"preserve existing bytes").unwrap();
    let result = run(&[
        "encryption",
        "remove",
        input.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
        "-p",
        "public secret",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(fs::read(&output).unwrap(), b"preserve existing bytes");
    assert_eq!(fs::read(input).unwrap(), original);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}
