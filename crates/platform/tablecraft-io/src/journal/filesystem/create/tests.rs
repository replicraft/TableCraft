use super::{CreateFailure, CreatePhase, create};
use crate::{
    IoError, SharedFile,
    journal::filesystem::{OfflineTarget, PairError},
};
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const INTENT: &[u8] = b"original-resolved-intent";
const TARGET: &[u8] = b"original-target-bytes";
const ALLOWANCE: u64 = 1_000_000;

struct Fixture {
    root: tempfile::TempDir,
    path: PathBuf,
    target: OfflineTarget,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(TARGET).unwrap();
        let target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
        Self { root, path, target }
    }

    fn bundle(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }

    fn assert_target_unchanged(&mut self) {
        assert_eq!(fs::read(&self.path).unwrap(), TARGET);
        assert!(matches!(
            SharedFile::open(&self.path),
            Err(IoError::LockBusy)
        ));
        let mut bytes = Vec::new();
        self.target.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, TARGET);
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(ALLOWANCE, 0, ALLOWANCE)
}

fn must_fail(result: Result<super::ExistingBundle, CreateFailure>) -> CreateFailure {
    match result {
        Ok(_) => panic!("unexpected setup success"),
        Err(error) => error,
    }
}

fn expected_binding(target: &Path, bundle: &Path, actual: &[u8]) -> Vec<u8> {
    // Independent literal wire layout: actual runtime object IDs and nonce are
    // observations, not copied producer framing/digest or decoded metadata.
    assert_eq!(actual.len(), 184);
    let mut expected = b"TCJPAIR1".to_vec();
    expected.extend(1_u32.to_le_bytes());
    expected.extend(1_u32.to_le_bytes());
    expected.extend(&actual[16..32]);
    for path in [
        target.to_path_buf(),
        bundle.to_path_buf(),
        bundle.join("binding"),
        bundle.join("intent"),
        bundle.join("journal"),
    ] {
        let metadata = fs::metadata(path).unwrap();
        expected.extend(metadata.dev().to_le_bytes());
        expected.extend(metadata.ino().to_le_bytes());
    }
    expected.extend((INTENT.len() as u64).to_le_bytes());
    expected.extend(Sha256::digest(INTENT));
    assert_eq!(expected.len(), 152);
    let checksum = Sha256::digest(&expected);
    expected.extend(checksum);
    expected
}

#[test]
fn fresh_factory_retains_original_handles_moved_intent_and_complete_binding() {
    let mut fixture = Fixture::new();
    let path = fixture.bundle("active");
    let intent = INTENT.to_vec();
    let allocation = intent.as_ptr();
    let bundle = create(&mut fixture.target, &path, intent, &mut budget(), &|| false)
        .unwrap_or_else(|failure| panic!("unexpected setup failure: {}", failure.cause));

    assert_eq!(bundle.intent_bytes.as_ptr(), allocation);
    assert_eq!(bundle.intent_bytes, INTENT);
    assert_eq!(fs::read(path.join("intent")).unwrap(), INTENT);
    assert_eq!(fs::metadata(path.join("journal")).unwrap().len(), 0);
    let wire = fs::read(path.join("binding")).unwrap();
    assert_eq!(wire, expected_binding(&fixture.path, &path, &wire));
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for name in ["binding", "intent", "journal"] {
        assert_eq!(
            fs::metadata(path.join(name)).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert!(matches!(
        SharedFile::open(&path.join("journal")).as_ref(),
        Err(IoError::LockBusy)
    ));
    fixture.assert_target_unchanged();
    drop(bundle);
    assert_eq!(fs::read(path.join("binding")).unwrap(), wire);
    assert!(SharedFile::open(&path.join("journal")).is_ok());
}

#[test]
fn cancellation_at_every_checkpoint_retains_partial_namespace_without_target_writes() {
    let mut fixture = Fixture::new();
    let calls = Cell::new(0);
    let path = fixture.bundle("measure");
    let complete = create(
        &mut fixture.target,
        &path,
        INTENT.to_vec(),
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap_or_else(|failure| panic!("unexpected setup failure: {}", failure.cause));
    drop(complete);
    let checkpoints = calls.get();
    assert!(checkpoints > 10);

    for stop in 0..checkpoints {
        let mut fixture = Fixture::new();
        let path = fixture.bundle("active");
        let observed = Cell::new(0);
        let intent = INTENT.to_vec();
        let allocation = intent.as_ptr();
        let failure = must_fail(create(
            &mut fixture.target,
            &path,
            intent,
            &mut budget(),
            &|| {
                let current = observed.get();
                observed.set(current + 1);
                current == stop
            },
        ));
        assert!(matches!(failure.cause, PairError::Io(IoError::Cancelled)));
        let existed = path.exists();
        if existed {
            let partial = failure
                .incomplete
                .first()
                .expect("created namespace retained");
            assert_ne!(partial.phase, CreatePhase::Captured);
            assert_eq!(partial.intent_bytes.as_ptr(), allocation);
        }
        drop(failure);
        assert_eq!(
            path.exists(),
            existed,
            "Drop must never delete setup entries"
        );
        fixture.assert_target_unchanged();
    }
}

#[test]
fn occupied_names_and_symlinks_are_never_replaced_or_removed() {
    for kind in ["file", "directory", "symlink"] {
        let mut fixture = Fixture::new();
        let path = fixture.bundle("occupied");
        match kind {
            "file" => fs::write(&path, b"foreign").unwrap(),
            "directory" => {
                fs::create_dir(&path).unwrap();
                fs::write(path.join("sentinel"), b"foreign").unwrap();
            }
            "symlink" => symlink(&fixture.path, &path).unwrap(),
            _ => unreachable!(),
        }
        let original = fs::symlink_metadata(&path).unwrap();
        let failure = must_fail(create(
            &mut fixture.target,
            &path,
            INTENT.to_vec(),
            &mut budget(),
            &|| false,
        ));
        assert!(matches!(
            failure.cause,
            PairError::Io(IoError::Io(std::io::ErrorKind::AlreadyExists))
        ));
        drop(failure);
        let after = fs::symlink_metadata(&path).unwrap();
        assert_eq!((original.dev(), original.ino()), (after.dev(), after.ino()));
        if kind == "directory" {
            assert_eq!(fs::read(path.join("sentinel")).unwrap(), b"foreign");
        }
        fixture.assert_target_unchanged();
    }
}

#[test]
fn factory_exact_byte_step_budget_and_each_one_below_are_enforced() {
    let mut fixture = Fixture::new();
    let mut measured = budget();
    let path = fixture.bundle("bundle-a");
    let bundle = create(
        &mut fixture.target,
        &path,
        INTENT.to_vec(),
        &mut measured,
        &|| false,
    )
    .unwrap_or_else(|failure| panic!("unexpected setup failure: {}", failure.cause));
    drop(bundle);
    let bytes = ALLOWANCE - measured.remaining(Resource::Bytes);
    let steps = ALLOWANCE - measured.remaining(Resource::Steps);
    assert!(bytes > 0 && steps > 0);

    let path = fixture.bundle("bundle-b");
    let mut exact = ResourceBudget::new(bytes, 0, steps);
    let bundle = create(
        &mut fixture.target,
        &path,
        INTENT.to_vec(),
        &mut exact,
        &|| false,
    )
    .unwrap_or_else(|failure| panic!("exact setup allowance failed: {}", failure.cause));
    drop(bundle);
    assert_eq!(exact.remaining(Resource::Bytes), 0);
    assert_eq!(exact.remaining(Resource::Steps), 0);

    for (name, bytes, steps, resource) in [
        ("bundle-c", bytes - 1, steps, Resource::Bytes),
        ("bundle-d", bytes, steps - 1, Resource::Steps),
    ] {
        let path = fixture.bundle(name);
        let failure = must_fail(create(
            &mut fixture.target,
            &path,
            INTENT.to_vec(),
            &mut ResourceBudget::new(bytes, 0, steps),
            &|| false,
        ));
        assert!(
            matches!(failure.cause, PairError::Io(IoError::Limit(error)) if error.resource == resource)
        );
        drop(failure);
    }
    fixture.assert_target_unchanged();

    // A one-below owner-slot allowance fails before even the target baseline
    // callback. A released target would otherwise refuse that callback first.
    let mut fixture = Fixture::new();
    fixture.target.release().unwrap();
    let path = fixture.bundle("preallocation");
    let mut insufficient = ResourceBudget::new(
        size_of::<super::IncompleteBundle>() as u64 - 1,
        0,
        ALLOWANCE,
    );
    let failure = must_fail(create(
        &mut fixture.target,
        &path,
        INTENT.to_vec(),
        &mut insufficient,
        &|| false,
    ));
    assert!(matches!(
        failure.cause,
        PairError::Io(IoError::Limit(error)) if error.resource == Resource::Bytes
    ));
    assert!(failure.incomplete.is_empty());
    assert!(!path.exists());
    assert_eq!(fs::read(fixture.path).unwrap(), TARGET);
}
