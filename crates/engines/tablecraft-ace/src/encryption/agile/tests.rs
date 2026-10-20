use super::*;
use tablecraft_core::limits::ResourceBudget;

fn xml() -> String {
    // Original inert descriptor with zero ciphertext, not a password success vector.
    r#"<encryption xmlns="http://schemas.microsoft.com/office/2006/encryption" xmlns:p="http://schemas.microsoft.com/office/2006/keyEncryptor/password"><keyData saltSize="16" blockSize="16" keyBits="128" hashSize="20" cipherAlgorithm="AES" cipherChaining="ChainingModeCBC" hashAlgorithm="SHA1" saltValue="AAAAAAAAAAAAAAAAAAAAAA=="/><keyEncryptors><keyEncryptor uri="http://schemas.microsoft.com/office/2006/keyEncryptor/password"><p:encryptedKey spinCount="0" saltSize="16" blockSize="16" keyBits="128" hashSize="20" cipherAlgorithm="AES" cipherChaining="ChainingModeCBC" hashAlgorithm="SHA1" saltValue="AAAAAAAAAAAAAAAAAAAAAA==" encryptedVerifierHashInput="AAAAAAAAAAAAAAAAAAAAAA==" encryptedVerifierHashValue="AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=" encryptedKeyValue="AAAAAAAAAAAAAAAAAAAAAA=="/></keyEncryptor></keyEncryptors></encryption>"#.into()
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

#[test]
fn descriptor_validation_resolves_namespaces_and_checks_exact_parameters() {
    let data = xml();
    let descriptor = descriptor::parse(data.as_bytes(), &mut budget(), &|| false).unwrap();
    assert_eq!(descriptor.data.key_bits, 128);
    assert_eq!(descriptor.password.hash, AgileHashAlgorithm::Sha1);
    assert_eq!(descriptor.spins, 0);
    let renamed = data
        .replace("xmlns:p=", "xmlns:q=")
        .replace("p:encryptedKey", "q:encryptedKey");
    assert!(descriptor::parse(renamed.as_bytes(), &mut budget(), &|| false).is_ok());
    let whitespace_salt = data.replace("AAAAAAAAAAAAAAAAAAAAAA==", "AAAA&#xA;AAAAAAAAAAAAAAAAAA==");
    assert!(descriptor::parse(whitespace_salt.as_bytes(), &mut budget(), &|| false).is_ok());
}

#[test]
fn wrong_namespaces_algorithms_duplicates_entities_and_missing_fields_are_refused() {
    let original = xml();
    for changed in [
        original.replace("hashSize=\"20\"", "hashSize=\"32\""),
        original.replace("spinCount=\"0\"", "spinCount=\"10000001\""),
        original.replace("keyBits=\"128\"", "keyBits=\"129\""),
        original.replace("ChainingModeCBC", "ChainingModeGCM"),
        original.replace("SHA1", "MD5"),
        original.replace("saltSize=\"16\"", "saltSize=\"15\""),
        original.replace("blockSize=\"16\"", "blockSize=\"8\""),
        original.replace("saltSize=\"16\"", "saltSize=\"16\" saltSize=\"16\""),
        original.replace("xmlns:p=", "xmlns:z="),
        original.replace("/keyEncryptor/password\"", "/keyEncryptor/certificate\""),
        original.replace("<keyData ", "<keyData unknown=\"1\" "),
        original.replace("<keyData ", "<keyData xmlns=\"urn:untrusted\" "),
        original.replace("hashAlgorithm=\"SHA1\"", ""),
        original.replace("spinCount=\"0\"", "spinCount=\"-1\""),
        original.replace("AAAAAAAAAAAAAAAAAAAAAA==", "not-base64"),
        format!("<!DOCTYPE encryption SYSTEM 'https://invalid.example/secret'>{original}"),
        format!("{original}{original}"),
        original.replace("</encryption>", ""),
        original.replace("</encryption>", "unexpected</encryption>"),
    ] {
        assert!(descriptor::parse(changed.as_bytes(), &mut budget(), &|| false).is_err());
    }
}

#[test]
fn descriptor_limits_cancellation_and_credentials_fail_without_secret_messages() {
    let input = xml();
    assert!(matches!(
        descriptor::parse(
            input.as_bytes(),
            &mut ResourceBudget::new(0, 0, 100),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
    assert!(matches!(
        descriptor::parse(input.as_bytes(), &mut budget(), &|| true),
        Err(InspectError::Cancelled)
    ));
    let d = descriptor::parse(input.as_bytes(), &mut budget(), &|| false).unwrap();
    let encryption = AgileEncryption {
        descriptor: d,
        encoding_key: Zeroizing::new([1, 2, 3, 4]),
        page_count: 3,
    };
    assert!(matches!(
        encryption.unlock(None, &mut budget(), &|| false),
        Err(InspectError::PasswordRequired)
    ));
    assert!(matches!(
        encryption.unlock(Some("secret"), &mut budget(), &|| false),
        Err(InspectError::IncorrectPassword)
    ));
    assert!(matches!(
        encryption.unlock(Some(""), &mut budget(), &|| false),
        Err(InspectError::IncorrectPassword)
    ));
    assert!(matches!(
        encryption.unlock(Some(&"x".repeat(256)), &mut budget(), &|| false),
        Err(InspectError::InvalidPasswordLength)
    ));
    assert!(matches!(
        encryption.unlock(Some(&"😀".repeat(128)), &mut budget(), &|| false),
        Err(InspectError::InvalidPasswordLength)
    ));
}

#[test]
fn password_iteration_work_is_cumulative_and_cancellable() {
    let input = xml().replace("spinCount=\"0\"", "spinCount=\"100\"");
    let d = descriptor::parse(input.as_bytes(), &mut budget(), &|| false).unwrap();
    let encryption = AgileEncryption {
        descriptor: d,
        encoding_key: Zeroizing::new([1, 2, 3, 4]),
        page_count: 3,
    };
    let mut short = ResourceBudget::new(100_000, 0, 20);
    assert!(matches!(
        encryption.unlock(Some("public test"), &mut short, &|| false),
        Err(InspectError::Limit(_))
    ));
    assert_eq!(short.remaining(Resource::Steps), 0);
    let calls = std::cell::Cell::new(0);
    assert!(matches!(
        encryption.unlock(Some("public test"), &mut budget(), &|| {
            calls.set(calls.get() + 1);
            calls.get() > 20
        }),
        Err(InspectError::Cancelled)
    ));
}

proptest::proptest! {
    #[test]
    fn arbitrary_xml_is_bounded_and_does_not_panic(data in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..=4096)) {
        let _ = descriptor::parse(&data, &mut budget(), &|| false);
    }
}
