use super::{Fixture, image, run};

const INFO_LENGTH_OFFSET: usize = 0x299;
const INFO_OFFSET: usize = INFO_LENGTH_OFFSET + size_of::<u16>();

fn encrypted_image() -> Vec<u8> {
    let mut bytes = image();
    bytes[62] ^= 1;
    let descriptor = b"\x04\0\x04\0\x40\0\0\0<private-salt-and-verifier/>";
    bytes[INFO_LENGTH_OFFSET..INFO_OFFSET]
        .copy_from_slice(&(descriptor.len() as u16).to_le_bytes());
    bytes[INFO_OFFSET..INFO_OFFSET + descriptor.len()].copy_from_slice(descriptor);
    bytes
}

#[test]
fn encryption_inspection_exposes_envelope_metadata_without_payloads_or_read_qualification() {
    let bytes = encrypted_image();
    let fixture = Fixture::new(&bytes);
    let output = run(&["encryption", "inspect", fixture.path()]);
    assert!(output.status.success(), "{:?}", output);
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Envelope version\t4.4\n"));
    assert!(text.contains("Envelope family\tAgile\n"));
    assert!(text.contains("Cipher configuration\tNot validated\n"));
    assert!(text.contains("Password verification\tNot performed\n"));
    assert!(!text.contains("private"));
    assert!(!text.contains("Outer flags"));
    let basic = run(&["inspect", fixture.path()]);
    assert!(basic.status.success());
    let summary = String::from_utf8(basic.stdout).unwrap();
    assert!(summary.contains("Page encryption\tYes\n"));
    assert!(summary.contains("Encryption type\tOffice Agile\n"));
    assert!(!summary.contains("Descriptor bytes"));
    assert!(!summary.contains("Envelope version"));
    let catalog = run(&["inspect", fixture.path(), "--catalog"]);
    assert_eq!(catalog.status.code(), Some(2));
    assert!(catalog.stdout.is_empty());
    assert_eq!(std::fs::read(&fixture.0).unwrap(), bytes);
    let table = run(&["--format", "table", "encryption", "inspect", fixture.path()]);
    assert!(table.status.success());
    assert!(
        String::from_utf8(table.stdout)
            .unwrap()
            .contains("Envelope family")
    );
}

#[test]
fn unencoded_pages_do_not_assert_absence_of_password_protection() {
    let fixture = Fixture::new(&image());
    let output = run(&["encryption", "inspect", fixture.path()]);
    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        b"Page encoding\tUnencoded\nPassword protection\tNot determined\n"
    );
}

#[test]
fn malformed_envelopes_fail_atomically_and_conflicting_modes_are_rejected() {
    let mut bytes = encrypted_image();
    bytes[INFO_OFFSET + 4] = 0x41;
    let fixture = Fixture::new(&bytes);
    let output = run(&["encryption", "inspect", fixture.path()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("malformed database encryption envelope")
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private"));
    for extra in [
        &["--catalog"][..],
        &["--relationships"],
        &["--table", "T"],
        &["--query", "Q"],
    ] {
        let mut args = vec!["encryption", "inspect", fixture.path()];
        args.extend(extra);
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    let removed_flag = run(&["inspect", fixture.path(), "--encryption"]);
    assert_eq!(removed_flag.status.code(), Some(2));
    assert!(removed_flag.stdout.is_empty());
    assert_eq!(std::fs::read(&fixture.0).unwrap(), bytes);
}
