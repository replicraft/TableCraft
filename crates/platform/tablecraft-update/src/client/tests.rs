use super::{LATEST_RELEASE, check};
use crate::artifact::ArtifactKind;
use crate::{
    UpdateError,
    http::{Body, Transport},
    platform::Platform,
};
use semver::Version;
use std::{
    cell::Cell,
    io::{self, Cursor},
};

struct Response {
    requested: Cell<bool>,
    length: Option<u64>,
    bytes: Vec<u8>,
}

impl Transport for Response {
    fn get(&self, url: &str) -> Result<Body, UpdateError> {
        assert_eq!(url, LATEST_RELEASE);
        self.requested.set(true);
        Ok(Body {
            reader: Box::new(Cursor::new(self.bytes.clone())),
            length: self.length,
        })
    }
}

#[test]
fn cancellation_prevents_network_and_declared_or_streamed_oversize_refuse() {
    let mut source = Response {
        requested: Cell::new(false),
        length: None,
        bytes: Vec::new(),
    };
    let current = Version::new(0, 1, 0);
    let platform = Platform::MacosArm64;
    assert!(matches!(
        check(
            &source,
            &current,
            platform,
            ArtifactKind::PortableArchive,
            &|| true
        ),
        Err(UpdateError::Cancelled)
    ));
    assert!(!source.requested.get());

    source.length = Some(super::release::MAX_METADATA_BYTES as u64 + 1);
    assert!(matches!(
        check(
            &source,
            &current,
            platform,
            ArtifactKind::PortableArchive,
            &|| false
        ),
        Err(UpdateError::ReleaseTooLarge)
    ));
    source.length = None;
    source.bytes = vec![b' '; super::release::MAX_METADATA_BYTES + 1];
    assert!(matches!(
        check(
            &source,
            &current,
            platform,
            ArtifactKind::PortableArchive,
            &|| false
        ),
        Err(UpdateError::ReleaseTooLarge)
    ));
}

#[test]
fn bounded_reader_checks_cancellation_between_chunks_and_surfaces_read_failure() {
    let cancelled = Cell::new(false);
    let mut reader = Cursor::new(vec![b' '; 70_000]);
    let result =
        crate::verification::bounded_bytes(&mut reader, 100_000, &mut || cancelled.replace(true));
    assert!(matches!(result, Err(UpdateError::Cancelled)));

    struct Broken;
    impl io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("independent read failure"))
        }
    }
    assert!(matches!(
        crate::verification::bounded_bytes(&mut Broken, 100, &mut || false),
        Err(UpdateError::Io(_))
    ));
}

#[test]
fn missing_latest_release_is_no_update_without_hiding_other_failures() {
    struct Missing;
    impl Transport for Missing {
        fn get(&self, _: &str) -> Result<Body, UpdateError> {
            Err(UpdateError::NotFound)
        }
    }
    assert!(
        check(
            &Missing,
            &Version::new(0, 1, 0),
            Platform::MacosArm64,
            ArtifactKind::PortableArchive,
            &|| false
        )
        .unwrap()
        .is_none()
    );
}
