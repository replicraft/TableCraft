use super::{ReopenError, reopen, reopen_location};
use crate::{
    IoError, SharedFile,
    journal::filesystem::target::{OfflineTarget, TargetLockState},
    namespace::Location,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const INTENT: &[u8] = b"resolved-intent";
const JOURNAL: &[u8] = b"retained-journal-bytes";
const CEILING: u64 = 1024;

struct Fixture {
    root: tempfile::TempDir,
    target_path: PathBuf,
    bundle: PathBuf,
    target: OfflineTarget,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

fn create_file(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
}

fn binding_wire(target: &Path, directory: &Path) -> Vec<u8> {
    // Independent filesystem source encoder. No production binding constants or
    // encoder are used; actual object identities come from independent metadata.
    let mut bytes = b"TCJPAIR1".to_vec();
    bytes.extend(1_u32.to_le_bytes());
    bytes.extend(1_u32.to_le_bytes());
    bytes.extend([0x42; 16]);
    let roles = [
        target.to_path_buf(),
        directory.to_path_buf(),
        directory.join("binding"),
        directory.join("intent"),
        directory.join("journal"),
    ];
    for path in roles {
        let metadata = fs::metadata(path).unwrap();
        bytes.extend(metadata.dev().to_le_bytes());
        bytes.extend(metadata.ino().to_le_bytes());
    }
    bytes.extend((INTENT.len() as u64).to_le_bytes());
    bytes.extend(Sha256::digest(INTENT));
    assert_eq!(bytes.len(), 152);
    let digest = Sha256::digest(&bytes);
    bytes.extend(digest);
    assert_eq!(bytes.len(), 184);
    bytes
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let target_path = root.path().join("target");
    create_file(&target_path, b"original");
    let bundle = root.path().join("active");
    fs::DirBuilder::new().mode(0o700).create(&bundle).unwrap();
    create_file(&bundle.join("binding"), b"");
    create_file(&bundle.join("intent"), INTENT);
    create_file(&bundle.join("journal"), JOURNAL);
    fs::write(bundle.join("binding"), binding_wire(&target_path, &bundle)).unwrap();
    let target = OfflineTarget::open(&target_path, &mut budget(), &|| false).unwrap();
    Fixture {
        root,
        target_path,
        bundle,
        target,
    }
}

fn unchanged(fixture: &Fixture) {
    assert_eq!(fs::read(&fixture.target_path).unwrap(), b"original");
    assert_eq!(fixture.target.lock_state(), TargetLockState::Held);
    assert!(matches!(
        SharedFile::open(&fixture.target_path),
        Err(IoError::LockBusy)
    ));
}

#[test]
fn qualify_existing_bundle_identity_and_opaque_intent_without_target_writes() {
    let mut fixture = fixture();
    let binding = fs::read(fixture.bundle.join("binding")).unwrap();
    let opened = reopen(
        &fixture.target,
        &fixture.bundle,
        CEILING,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(opened.intent_bytes, INTENT);
    assert!(opened.binding.target == fixture.target.identity());
    assert!(matches!(
        SharedFile::open(&fixture.bundle.join("journal")),
        Err(IoError::LockBusy)
    ));
    drop(opened);
    unchanged(&fixture);
    assert_eq!(fs::read(fixture.bundle.join("binding")).unwrap(), binding);
    assert_eq!(fs::read(fixture.bundle.join("intent")).unwrap(), INTENT);
    assert_eq!(fs::read(fixture.bundle.join("journal")).unwrap(), JOURNAL);
    fixture.target.release().unwrap();
    assert_eq!(fs::read_dir(fixture.root.path()).unwrap().count(), 2);
}

#[test]
fn qualify_private_bundle_and_child_modes_before_reopen_handoff() {
    for role in [None, Some("binding"), Some("intent"), Some("journal")] {
        let mut fixture = fixture();
        let path = role.map_or_else(|| fixture.bundle.clone(), |name| fixture.bundle.join(name));
        let original_mode = if role.is_none() { 0o700 } else { 0o600 };
        let shared_mode = if role.is_none() { 0o755 } else { 0o644 };
        fs::set_permissions(&path, fs::Permissions::from_mode(shared_mode)).unwrap();
        assert!(matches!(
            reopen(
                &fixture.target,
                &fixture.bundle,
                CEILING,
                &mut budget(),
                &|| false
            ),
            Err(ReopenError::Io(IoError::Io(ErrorKind::PermissionDenied)))
        ));
        unchanged(&fixture);
        assert_eq!(fs::read(fixture.bundle.join("intent")).unwrap(), INTENT);
        assert_eq!(fs::read(fixture.bundle.join("journal")).unwrap(), JOURNAL);

        // Only fixture metadata changes back; no record bytes are rewritten.
        fs::set_permissions(&path, fs::Permissions::from_mode(original_mode)).unwrap();
        drop(
            reopen(
                &fixture.target,
                &fixture.bundle,
                CEILING,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        unchanged(&fixture);
        fixture.target.release().unwrap();
    }
}

#[test]
fn bundle_reopen_rejects_different_equal_content_target_and_replaced_roles() {
    let mut fixture = fixture();
    let other_path = fixture.root.path().join("equal-copy");
    create_file(&other_path, b"original");
    let mut other = OfflineTarget::open(&other_path, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        reopen(&other, &fixture.bundle, CEILING, &mut budget(), &|| false),
        Err(ReopenError::IdentityMismatch)
    ));
    other.release().unwrap();

    fs::rename(
        fixture.bundle.join("journal"),
        fixture.bundle.join("old-journal"),
    )
    .unwrap();
    create_file(&fixture.bundle.join("journal"), JOURNAL);
    assert!(
        reopen(
            &fixture.target,
            &fixture.bundle,
            CEILING,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    unchanged(&fixture);
    fixture.target.release().unwrap();
}

#[test]
fn bundle_reopen_rejects_symlink_and_hardlink_target_alias_before_journal_adoption() {
    for as_symlink in [false, true] {
        let mut fixture = fixture();
        fs::remove_file(fixture.bundle.join("journal")).unwrap();
        if as_symlink {
            symlink(&fixture.target_path, fixture.bundle.join("journal")).unwrap();
        } else {
            fs::hard_link(&fixture.target_path, fixture.bundle.join("journal")).unwrap();
        }
        assert!(
            reopen(
                &fixture.target,
                &fixture.bundle,
                CEILING,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
        unchanged(&fixture);
        fixture.target.release().unwrap();
    }
}

#[test]
fn bundle_reopen_checks_intent_ceiling_and_exact_digest_without_mutation() {
    let mut fixture = fixture();
    assert!(matches!(
        reopen(
            &fixture.target,
            &fixture.bundle,
            INTENT.len() as u64 - 1,
            &mut budget(),
            &|| false
        ),
        Err(ReopenError::IntentLimit)
    ));
    assert!(
        reopen(
            &fixture.target,
            &fixture.bundle,
            INTENT.len() as u64,
            &mut budget(),
            &|| false
        )
        .is_ok()
    );
    let mut wrong = INTENT.to_vec();
    wrong[0] ^= 1;
    fs::write(fixture.bundle.join("intent"), &wrong).unwrap();
    assert!(matches!(
        reopen(
            &fixture.target,
            &fixture.bundle,
            CEILING,
            &mut budget(),
            &|| false
        ),
        Err(ReopenError::IntentMismatch)
    ));
    unchanged(&fixture);
    assert_eq!(fs::read(fixture.bundle.join("intent")).unwrap(), wrong);
    fixture.target.release().unwrap();
}

#[test]
fn bundle_reopen_holds_original_parent_when_text_selector_changes() {
    let mut fixture = fixture();
    let selector = fixture.root.path().join("selector");
    symlink(fixture.root.path(), &selector).unwrap();
    let location = Location::capture(&selector.join("active"), &mut budget(), &|| false).unwrap();
    let other = tempfile::tempdir().unwrap();
    fs::create_dir(other.path().join("active")).unwrap();
    create_file(&other.path().join("active/untouched"), b"other");
    fs::remove_file(&selector).unwrap();
    symlink(other.path(), &selector).unwrap();

    let opened =
        reopen_location(&fixture.target, location, CEILING, &mut budget(), &|| false).unwrap();
    assert_eq!(opened.intent_bytes, INTENT);
    drop(opened);
    assert_eq!(
        fs::read(other.path().join("active/untouched")).unwrap(),
        b"other"
    );
    assert_eq!(
        fs::read_dir(other.path().join("active")).unwrap().count(),
        1
    );
    unchanged(&fixture);
    fixture.target.release().unwrap();
}

#[test]
fn bundle_reopen_exact_pools_and_terminal_cancel_release_only_new_resources() {
    let mut fixture = fixture();
    let mut measured = budget();
    let polls = std::cell::Cell::new(0);
    drop(
        reopen(
            &fixture.target,
            &fixture.bundle,
            CEILING,
            &mut measured,
            &|| {
                polls.set(polls.get() + 1);
                false
            },
        )
        .unwrap(),
    );
    let resources = [Resource::Bytes, Resource::Steps];
    let used =
        resources.map(|resource| budget().remaining(resource) - measured.remaining(resource));
    for (position, resource) in resources.into_iter().enumerate() {
        for exact in [false, true] {
            let mut limits = [1_000_000, 1_000_000];
            limits[position] = used[position] - u64::from(!exact);
            let mut allowance = ResourceBudget::new(limits[0], 0, limits[1]);
            let result = reopen(
                &fixture.target,
                &fixture.bundle,
                CEILING,
                &mut allowance,
                &|| false,
            );
            assert_eq!(result.is_ok(), exact);
            if exact {
                assert_eq!(allowance.remaining(resource), 0);
            }
            drop(result);
            SharedFile::open(&fixture.bundle.join("journal"))
                .unwrap()
                .close()
                .unwrap();
            unchanged(&fixture);
        }
    }
    let calls = std::cell::Cell::new(0);
    let terminal = polls.get();
    assert!(
        reopen(
            &fixture.target,
            &fixture.bundle,
            CEILING,
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() == terminal
            }
        )
        .is_err()
    );
    SharedFile::open(&fixture.bundle.join("journal"))
        .unwrap()
        .close()
        .unwrap();
    unchanged(&fixture);
    fixture.target.release().unwrap();
}

#[test]
fn qualify_fifo_roles_refuse_without_waiting_for_a_peer_or_writing_target() {
    for role in ["binding", "intent", "journal"] {
        let mut fixture = fixture();
        let path = fixture.bundle.join(role);
        fs::remove_file(&path).unwrap();
        // Actual FIFO source: no reader/writer peer and no timeout can mask a
        // blocking open. The system utility only creates this fixture node.
        let status = std::process::Command::new("mkfifo")
            .arg("-m")
            .arg("600")
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(matches!(
            reopen(
                &fixture.target,
                &fixture.bundle,
                CEILING,
                &mut budget(),
                &|| false
            ),
            Err(ReopenError::Io(IoError::Io(ErrorKind::InvalidInput)))
        ));
        unchanged(&fixture);
        fixture.target.release().unwrap();
    }

    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("target-fifo");
    let status = std::process::Command::new("mkfifo")
        .arg("-m")
        .arg("600")
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(matches!(
        OfflineTarget::open(&path, &mut budget(), &|| false),
        Err(IoError::Io(ErrorKind::InvalidInput))
    ));
}
