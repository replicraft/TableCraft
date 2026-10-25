//! Explicit linked-target options use original inert synthetic fixture builders.
#[allow(dead_code)]
#[path = "../../../../crates/engines/tablecraft-ace/tests/support/agile.rs"]
mod agile_fixture;
#[allow(dead_code)]
#[path = "../../../../crates/engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
#[allow(dead_code)]
#[path = "../../../../crates/engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[allow(dead_code)]
#[path = "../../../../crates/engines/tablecraft-ace/tests/version_history/fixture.rs"]
mod history_fixture;

use super::{
    Database, linked_fixture, run, support, target_fixture as base, target_fixture as fixture,
};

const PAGE_BYTES: usize = 4096;
const HEADER_SORT_OFFSET: usize = 110;
const TABLE_DEFINITION_PAGE: usize = 10;
const TABLE_DATA_PAGE: usize = 12;
const FIELD_START: usize = 63;
const FIELD_BYTES: usize = 25;
const FIELD_SORT_OFFSET: usize = 11;

fn root() -> Vec<u8> {
    linked_fixture::database(
        6,
        Some("/not-opened/stored-target.accdb"),
        Some("Items"),
        None,
    )
}

#[test]
fn encrypted_target_uses_explicit_password_even_when_root_is_plaintext() {
    let root = Database::new(root());
    let mut plain = base::database();
    plain[4..20].copy_from_slice(b"Standard ACE DB\0");
    plain[20] = 6;
    let target = Database::new(agile_fixture::encrypted_image(
        &plain,
        6,
        "public linked password",
    ));
    let arguments = [
        "query",
        "run",
        "SELECT Id,Score FROM Linked ORDER BY Id",
        "--database",
        root.path(),
        "--link",
        "Linked",
        target.path(),
    ];
    let mut correct = arguments.to_vec();
    correct.extend(["-p", "public linked password"]);
    let output = run(&correct);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"Id\tScore\n1\t10\n2\t20\n3\t30\n");

    let mut wrong = arguments.to_vec();
    wrong.extend(["-p", "incorrect linked password"]);
    for arguments in [arguments.to_vec(), wrong] {
        // run() captures output with noninteractive stdin: omission must not hang
        // or publish a result before an encrypted target can be authenticated.
        let output = run(&arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .to_ascii_lowercase()
                .contains("password")
        );
    }
    // Database's drop guard compares both original files byte for byte.
}

#[test]
fn stored_calculation_opt_in_reaches_target_and_never_recomputes_cached_values() {
    let root = Database::new(root());
    let target = Database::new(calculated_fixture::database(&[Some(999), None, Some(41)]));
    let arguments = [
        "query",
        "run",
        "SELECT Id,Score FROM Linked ORDER BY Id",
        "--database",
        root.path(),
        "--link",
        "Linked",
        target.path(),
    ];
    let refused = run(&arguments);
    assert!(!refused.status.success());
    assert!(refused.stdout.is_empty());

    let mut allowed = arguments.to_vec();
    allowed.push("--stored-calculations");
    let output = run(&allowed);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"Id\tScore\n1\t999\n2\tNULL\n3\t41\n");
}

#[test]
fn collections_opt_in_reaches_target_history_members_beyond_multivalue_mode() {
    let root = Database::new(root());
    let target = Database::new(history_fixture::database());
    let arguments = [
        "query",
        "run",
        "SELECT Id,Count(Score.Notes) AS N FROM Linked GROUP BY Id ORDER BY Id",
        "--database",
        root.path(),
        "--link",
        "Linked",
        target.path(),
    ];
    for option in [None, Some("--multivalues")] {
        let mut refused = arguments.to_vec();
        if let Some(option) = option {
            refused.push(option);
        }
        let output = run(&refused);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let mut allowed = arguments.to_vec();
    allowed.push("--collections");
    let output = run(&allowed);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"Id\tN\n1\t2\n2\t1\n3\t0\n");
}

fn database_order(bytes: &mut [u8], order: [u8; 4]) {
    for (index, mask) in [1, 27, 152, 253].into_iter().enumerate() {
        bytes[HEADER_SORT_OFFSET + index] = order[index] ^ mask;
    }
}

fn text_target() -> Vec<u8> {
    let mut bytes = base::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    database_order(&mut bytes, [9, 4, 0, 1]);
    let mut definition = base::definition(&[("Id", 4, 0, 4), ("Score", 10, 0, 16)], 1, 11 << 8);
    let text_field = FIELD_START + FIELD_BYTES;
    definition[text_field + 15] = 2;
    definition[text_field + FIELD_SORT_OFFSET..text_field + FIELD_SORT_OFFSET + 4]
        .copy_from_slice(&[9, 4, 0, 1]);
    bytes[TABLE_DEFINITION_PAGE * PAGE_BYTES..(TABLE_DEFINITION_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&definition);

    let mut row = 2_u16.to_le_bytes().to_vec();
    row.extend(1_i32.to_le_bytes());
    let start = row.len() as u16;
    row.extend(0x80_u16.to_le_bytes());
    row.extend((row.len() as u16).to_le_bytes());
    row.extend(start.to_le_bytes());
    row.extend(1_u16.to_le_bytes());
    row.push(3);
    bytes[TABLE_DATA_PAGE * PAGE_BYTES..(TABLE_DATA_PAGE + 1) * PAGE_BYTES].copy_from_slice(
        &support::data_page(TABLE_DEFINITION_PAGE as u32, &[(0, row)]),
    );
    bytes
}

#[test]
fn database_policy_uses_target_field_order_and_root_literal_default_independently() {
    let mut root_bytes = root();
    database_order(&mut root_bytes, [9, 4, 0, 0]);
    let root = Database::new(root_bytes);
    let target = Database::new(text_target());
    // Existing independent General goldens order U+0080 below U+0301 in legacy,
    // above it in 2010. The root default is legacy; target field/default are 2010.
    // This is explicit TableCraft comparison-model evidence, not native SQL proof.
    for (policy, expected) in [
        (
            "database",
            b"Id\tFieldPolicy\tRootPolicy\n1\tTRUE\tTRUE\n".as_slice(),
        ),
        (
            "general-legacy",
            b"Id\tFieldPolicy\tRootPolicy\n1\tFALSE\tTRUE\n".as_slice(),
        ),
        (
            "general-2010",
            b"Id\tFieldPolicy\tRootPolicy\n1\tTRUE\tFALSE\n".as_slice(),
        ),
    ] {
        let output = run(&[
            "query",
            "run",
            "SELECT Id,Score>ChrW(769) AS FieldPolicy,ChrW(128)<ChrW(769) AS RootPolicy FROM Linked",
            "--database",
            root.path(),
            "--link",
            "Linked",
            target.path(),
            "--collation",
            policy,
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected, "{policy}");
    }
}
