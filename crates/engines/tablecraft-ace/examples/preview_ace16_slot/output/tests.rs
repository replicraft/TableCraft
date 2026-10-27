use super::*;
use crate::tests::Directory;

#[test]
fn failed_owned_partial_write_is_removed_but_preexisting_output_is_untouched() {
    let directory = Directory::new();
    let owned = directory.0.join("partial.accdb");
    let error = publish_with(&owned, b"complete candidate", |output, _| {
        output.write_all(b"partial")?;
        Err(io::Error::other(
            "injected write or synchronization failure",
        ))
    })
    .err()
    .unwrap();
    assert!(error.to_string().contains("injected"));
    assert!(!owned.exists());

    let existing = directory.file("existing.accdb", b"untouched");
    let mut called = false;
    assert!(
        publish_with(&existing, b"replacement", |_, _| {
            called = true;
            Ok(())
        })
        .is_err()
    );
    assert!(!called);
    assert_eq!(fs::read(existing).unwrap(), b"untouched");
}
