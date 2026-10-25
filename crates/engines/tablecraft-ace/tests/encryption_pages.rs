#[path = "support/agile.rs"]
mod fixture;

use std::{
    cell::Cell,
    fs,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
};
use tablecraft_ace::{EncryptionRewrite, InspectError, rewrite_encryption, rewrite_encryption_to};
use tablecraft_core::limits::ResourceBudget;

fn plain() -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 10_000_000)
}

struct PageReader<'a>(Cursor<&'a [u8]>);

impl Read for PageReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        assert!(bytes.len() <= 4096, "whole-image read attempted");
        self.0.read(bytes)
    }
}

impl Seek for PageReader<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.0.seek(from)
    }
}

#[test]
fn page_removal_matches_image_api_and_never_reads_an_image_buffer() {
    let original = plain();
    for code in [2, 3, 5, 6] {
        let encrypted = fixture::encrypted_image(&original, code, "public password");
        let operation = || EncryptionRewrite::Remove {
            password: Some("public password"),
        };
        let image = rewrite_encryption(
            &mut Cursor::new(&encrypted),
            operation(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut output = Cursor::new(Vec::new());
        rewrite_encryption_to(
            &mut PageReader(Cursor::new(&encrypted)),
            &mut output,
            operation(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            output.get_ref() == image.as_bytes(),
            "removal mismatch for code {code}"
        );
    }
}

#[test]
fn page_add_change_and_remove_recover_exact_original_content() {
    for code in [1, 2, 3, 5, 6] {
        let mut original = plain();
        original[20] = code;
        if code == 1 {
            original[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        let mut added = Cursor::new(Vec::new());
        rewrite_encryption_to(
            &mut PageReader(Cursor::new(&original)),
            &mut added,
            EncryptionRewrite::Add {
                password: None,
                new_password: "public password",
            },
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_ne!(&added.get_ref()[4096..], &original[4096..]);

        let mut changed = Cursor::new(Vec::new());
        rewrite_encryption_to(
            &mut PageReader(Cursor::new(added.get_ref())),
            &mut changed,
            EncryptionRewrite::ChangePassword {
                password: Some("public password"),
                new_password: "replacement",
            },
            &mut budget(),
            &|| false,
        )
        .unwrap();
        if code != 1 {
            assert_eq!(&changed.get_ref()[4096..], &added.get_ref()[4096..]);
        }

        let mut removed = Cursor::new(Vec::new());
        rewrite_encryption_to(
            &mut PageReader(Cursor::new(changed.get_ref())),
            &mut removed,
            EncryptionRewrite::Remove {
                password: Some("replacement"),
            },
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut expected = original;
        if code == 2 {
            // Existing Agile creation promotes ACE 2007 to the compatibility
            // envelope. Removal retains that required header promotion.
            expected[20..22].copy_from_slice(&[3, 1]);
        }
        assert!(
            removed.get_ref() == &expected,
            "round trip mismatch for code {code}"
        );
    }
}

struct FailingWriter {
    bytes: Cursor<Vec<u8>>,
    remaining: usize,
}

impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::ErrorKind::StorageFull.into());
        }
        let count = bytes.len().min(self.remaining).min(97);
        self.remaining -= count;
        self.bytes.write(&bytes[..count])
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("host owns flush and synchronization");
    }
}

impl Seek for FailingWriter {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(from)
    }
}

#[test]
fn partial_output_errors_and_cancellation_propagate_without_changing_source() {
    let original = fixture::encrypted_image(&plain(), 5, "public password");
    let operation = || EncryptionRewrite::Remove {
        password: Some("public password"),
    };
    for allowance in [0, 1, 4096, 8199] {
        let mut source = Cursor::new(original.clone());
        let mut output = FailingWriter {
            bytes: Cursor::new(Vec::new()),
            remaining: allowance,
        };
        let result = rewrite_encryption_to(
            &mut source,
            &mut output,
            operation(),
            &mut budget(),
            &|| false,
        );
        assert_eq!(result, Err(InspectError::Io(io::ErrorKind::StorageFull)));
        assert!(source.get_ref() == &original, "input source changed");
        assert_eq!(output.bytes.get_ref().len(), allowance);
    }

    let visits = Cell::new(0);
    rewrite_encryption_to(
        &mut Cursor::new(&original),
        &mut Cursor::new(Vec::new()),
        operation(),
        &mut budget(),
        &|| {
            visits.set(visits.get() + 1);
            false
        },
    )
    .unwrap();
    let total = visits.get();
    for boundary in [1, 100, total / 2, total - 1, total] {
        visits.set(0);
        assert_eq!(
            rewrite_encryption_to(
                &mut Cursor::new(&original),
                &mut Cursor::new(Vec::new()),
                operation(),
                &mut budget(),
                &|| {
                    visits.set(visits.get() + 1);
                    visits.get() == boundary
                },
            ),
            Err(InspectError::Cancelled),
            "boundary {boundary}",
        );
    }
}

#[test]
fn nonempty_target_and_wrong_credentials_are_refused_before_writes() {
    let encrypted = fixture::encrypted_image(&plain(), 5, "public password");
    let mut occupied = Cursor::new(b"existing output".to_vec());
    assert_eq!(
        rewrite_encryption_to(
            &mut Cursor::new(&encrypted),
            &mut occupied,
            EncryptionRewrite::Remove { password: None },
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::Io(io::ErrorKind::AlreadyExists)),
    );
    assert_eq!(occupied.into_inner(), b"existing output");

    let mut empty = Cursor::new(Vec::new());
    assert_eq!(
        rewrite_encryption_to(
            &mut Cursor::new(&encrypted),
            &mut empty,
            EncryptionRewrite::Remove {
                password: Some("wrong")
            },
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::IncorrectPassword),
    );
    assert!(empty.into_inner().is_empty());
}

struct RevisedHeader<'a> {
    source: Cursor<&'a [u8]>,
    written: &'a Cell<u64>,
}

impl Read for RevisedHeader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let start = self.source.position();
        let count = self.source.read(bytes)?;
        if self.written.get() == self.source.get_ref().len() as u64
            && (start..start + count as u64).contains(&1000)
        {
            bytes[(1000 - start) as usize] ^= 1;
        }
        Ok(count)
    }
}

impl Seek for RevisedHeader<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.source.seek(from)
    }
}

struct TrackedTarget<'a> {
    target: Cursor<Vec<u8>>,
    written: &'a Cell<u64>,
}

impl Write for TrackedTarget<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.target.write(bytes)?;
        self.written.set(self.target.position());
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for TrackedTarget<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.target.seek(from)
    }
}

#[test]
fn final_full_header_revision_refuses_even_after_all_output_pages_were_written() {
    let original = fixture::encrypted_image(&plain(), 5, "public password");
    let written = Cell::new(0);
    let mut source = RevisedHeader {
        source: Cursor::new(&original),
        written: &written,
    };
    let mut output = TrackedTarget {
        target: Cursor::new(Vec::new()),
        written: &written,
    };
    assert_eq!(
        rewrite_encryption_to(
            &mut source,
            &mut output,
            EncryptionRewrite::Remove {
                password: Some("public password")
            },
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::Malformed),
    );
    assert_eq!(written.get(), original.len() as u64);

    let mut empty = Cursor::new(Vec::new());
    assert!(
        rewrite_encryption_to(
            &mut Cursor::new(&original),
            &mut empty,
            EncryptionRewrite::Remove {
                password: Some("public password")
            },
            &mut ResourceBudget::new(0, 0, 0),
            &|| false,
        )
        .is_err()
    );
    assert!(empty.into_inner().is_empty());
}
