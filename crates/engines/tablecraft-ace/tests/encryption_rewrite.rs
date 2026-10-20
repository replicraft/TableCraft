#[path = "support/agile.rs"]
mod fixture;
use std::{
    cell::Cell,
    fs,
    io::{self, Cursor, Read, Seek, SeekFrom},
    path::Path,
};
use tablecraft_ace::{EncryptionRewrite, InspectError, rewrite_encryption};
use tablecraft_core::limits::ResourceBudget;

fn image() -> Vec<u8> {
    let plain = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    fixture::encrypted_image(&plain, 5, "public password")
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 10_000_000)
}

#[test]
fn cancellation_at_intermediate_and_final_publication_checks_never_returns_a_candidate() {
    let image = image();
    let mut source = Cursor::new(&image);
    let visits = Cell::new(0);
    let operation = || EncryptionRewrite::Remove {
        password: Some("public password"),
    };
    rewrite_encryption(&mut source, operation(), &mut budget(), &|| {
        visits.set(visits.get() + 1);
        false
    })
    .unwrap();
    let final_visit = visits.get();
    assert!(final_visit > 1_000);
    for boundary in [1, 10, 100, 1_000, final_visit] {
        visits.set(0);
        let result = rewrite_encryption(&mut source, operation(), &mut budget(), &|| {
            visits.set(visits.get() + 1);
            visits.get() == boundary
        });
        assert!(
            matches!(result, Err(InspectError::Cancelled)),
            "boundary {boundary}"
        );
        assert_eq!(source.get_ref(), &&image);
    }
    assert!(
        rewrite_encryption(
            &mut source,
            operation(),
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        )
        .is_err()
    );
    assert_eq!(source.into_inner(), &image);
}

struct LargeSource {
    bytes: Cursor<Vec<u8>>,
}
impl Read for LargeSource {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(bytes)
    }
}
impl Seek for LargeSource {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        if matches!(from, SeekFrom::End(0)) {
            Ok(65 * 1024 * 1024)
        } else {
            self.bytes.seek(from)
        }
    }
}
#[test]
fn rewrite_image_cap_is_separate_from_the_larger_streaming_read_envelope() {
    let image = image();
    let mut source = LargeSource {
        bytes: Cursor::new(image[..4096].to_vec()),
    };
    let result = rewrite_encryption(
        &mut source,
        EncryptionRewrite::Remove {
            password: Some("public password"),
        },
        &mut budget(),
        &|| false,
    );
    assert!(matches!(result, Err(InspectError::StructureLimit)));
    assert_eq!(source.bytes.into_inner(), image[..4096]);
}

#[test]
fn add_checks_cancellation_through_key_derivation_remasking_and_final_publication() {
    let image = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    let mut source = Cursor::new(&image);
    let visits = Cell::new(0);
    let operation = || EncryptionRewrite::Add {
        password: None,
        new_password: "public password",
    };
    rewrite_encryption(&mut source, operation(), &mut budget(), &|| {
        visits.set(visits.get() + 1);
        false
    })
    .unwrap();
    let final_visit = visits.get();
    assert!(final_visit > 100_000);
    for boundary in [1, 10, 1000, 100_000, final_visit] {
        visits.set(0);
        let result = rewrite_encryption(&mut source, operation(), &mut budget(), &|| {
            visits.set(visits.get() + 1);
            visits.get() == boundary
        });
        assert!(
            matches!(result, Err(InspectError::Cancelled)),
            "boundary {boundary}"
        );
        assert_eq!(source.get_ref(), &&image);
    }
    let mut large = LargeSource {
        bytes: Cursor::new(image[..4096].to_vec()),
    };
    assert!(matches!(
        rewrite_encryption(&mut large, operation(), &mut budget(), &|| false),
        Err(InspectError::StructureLimit)
    ));
    assert!(
        rewrite_encryption(
            &mut source,
            operation(),
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        )
        .is_err()
    );
}
