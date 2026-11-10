use super::{DownloadProgress, download};
use crate::artifact::ArtifactKind;
use crate::{
    UpdateError,
    http::{Body, Transport},
    platform::Platform,
    release, trust,
};
use semver::Version;
use serde_json::json;
use std::{
    cell::RefCell,
    collections::VecDeque,
    fs,
    io::{Cursor, Read},
    path::Path,
};

const PAYLOAD: &[u8] = include_bytes!("tests/payload.txt");
const SIGNATURE: &[u8] = include_bytes!("tests/payload.minisig");
const PUBLIC: &str = include_str!("tests/TEST-ONLY.pub");

struct StoredResponse {
    url: String,
    bytes: Vec<u8>,
    length: Option<u64>,
}

struct Responses(RefCell<VecDeque<StoredResponse>>);

impl Transport for Responses {
    fn get(&self, url: &str) -> Result<Body, UpdateError> {
        let StoredResponse {
            url: expected,
            bytes,
            length,
        } = self.0.borrow_mut().pop_front().expect("unexpected request");
        assert_eq!(url, expected, "signature must be retrieved before archive");
        Ok(Body {
            reader: Box::new(Cursor::new(bytes)),
            length,
        })
    }
}

fn available() -> release::AvailableUpdate {
    let name = "tablecraft-desktop-0.1.1-macos-arm64.zip";
    let base = "https://github.com/replicraft/TableCraft/releases/download/v0.1.1";
    let metadata = json!({
        "tag_name": "v0.1.1", "draft": false, "prerelease": false,
        "html_url": "https://github.com/replicraft/TableCraft/releases/tag/v0.1.1",
        "assets": [
            { "name": name, "size": PAYLOAD.len(), "browser_download_url": format!("{base}/{name}") },
            { "name": format!("{name}.minisig"), "size": SIGNATURE.len(), "browser_download_url": format!("{base}/{name}.minisig") }
        ]
    });
    release::select(
        &serde_json::to_vec(&metadata).unwrap(),
        &Version::new(0, 1, 0),
        Platform::MacosArm64,
        ArtifactKind::PortableArchive,
    )
    .unwrap()
    .unwrap()
}

fn responses(
    release: &release::AvailableUpdate,
    payload: Vec<u8>,
    signature: Vec<u8>,
) -> Responses {
    Responses(RefCell::new(VecDeque::from([
        StoredResponse {
            url: release.signature_url().to_owned(),
            bytes: signature,
            length: None,
        },
        StoredResponse {
            url: release.asset_url().to_owned(),
            bytes: payload,
            length: None,
        },
    ])))
}

fn no_staging(parent: &Path) {
    assert_eq!(
        fs::read_dir(parent).unwrap().count(),
        0,
        "failed download retained staging"
    );
}

#[test]
fn authenticated_download_has_exact_bytes_rechecks_mutation_and_cleans_owned_directory() {
    let parent = tempfile::tempdir().unwrap();
    let release = available();
    let source = responses(&release, PAYLOAD.to_vec(), SIGNATURE.to_vec());
    let key = trust::parse_key(PUBLIC).unwrap();
    let mut seen = Vec::new();
    let verified = download(&source, &key, &release, parent.path(), &mut |progress| {
        seen.push(progress);
        true
    })
    .unwrap();
    assert_eq!(verified.version(), &Version::new(0, 1, 1));
    assert_eq!(verified.asset_name(), release.asset_name());
    assert_eq!(
        seen.last(),
        Some(&DownloadProgress {
            received: PAYLOAD.len() as u64,
            total: PAYLOAD.len() as u64
        })
    );
    let mut reader = verified.open_verified(&|| false).unwrap();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, PAYLOAD);
    drop(reader);
    fs::write(verified.file.path(), vec![b'X'; PAYLOAD.len()]).unwrap();
    assert!(matches!(
        verified.open_verified(&|| false),
        Err(UpdateError::PayloadChanged)
    ));
    drop(verified);
    no_staging(parent.path());
}

#[test]
fn modified_short_or_appended_archive_refuses_even_without_content_length() {
    let parent = tempfile::tempdir().unwrap();
    let release = available();
    let key = trust::parse_key(PUBLIC).unwrap();
    let mut changed = PAYLOAD.to_vec();
    changed[0] ^= 1;
    for payload in [
        changed,
        PAYLOAD[..PAYLOAD.len() - 1].to_vec(),
        [PAYLOAD, b"extra"].concat(),
    ] {
        let source = responses(&release, payload, SIGNATURE.to_vec());
        let result = download(&source, &key, &release, parent.path(), &mut |_| true);
        assert!(matches!(
            result,
            Err(UpdateError::Signature | UpdateError::LengthMismatch)
        ));
        no_staging(parent.path());
    }
}

#[test]
fn wrong_trusted_identity_or_key_refuses_and_cancellation_never_publishes() {
    let parent = tempfile::tempdir().unwrap();
    let release = available();
    let key = trust::parse_key(PUBLIC).unwrap();
    let changed = str::from_utf8(SIGNATURE)
        .unwrap()
        .replace("tablecraft-update 0.1.1", "tablecraft-update 0.1.2");
    let source = responses(&release, PAYLOAD.to_vec(), changed.into_bytes());
    assert!(matches!(
        download(&source, &key, &release, parent.path(), &mut |_| true),
        Err(UpdateError::Signature)
    ));
    assert_eq!(
        source.0.borrow().len(),
        1,
        "invalid identity must not fetch payload"
    );
    no_staging(parent.path());

    let source = responses(&release, PAYLOAD.to_vec(), SIGNATURE.to_vec());
    let mut updates = 0;
    assert!(matches!(
        download(&source, &key, &release, parent.path(), &mut |_| {
            updates += 1;
            updates < 4
        }),
        Err(UpdateError::Cancelled)
    ));
    no_staging(parent.path());
}

#[test]
fn false_content_length_refuses_and_duplicate_signature_lines_refuse() {
    let parent = tempfile::tempdir().unwrap();
    let release = available();
    let key = trust::parse_key(PUBLIC).unwrap();
    let source = responses(&release, PAYLOAD.to_vec(), SIGNATURE.to_vec());
    source.0.borrow_mut().back_mut().unwrap().length = Some(PAYLOAD.len() as u64 + 1);
    assert!(matches!(
        download(&source, &key, &release, parent.path(), &mut |_| true),
        Err(UpdateError::LengthMismatch)
    ));
    no_staging(parent.path());
    let extra = [SIGNATURE, b"unexpected fifth line\n"].concat();
    assert!(matches!(
        crate::verification::signature(&extra, &release),
        Err(UpdateError::Signature)
    ));
}

#[test]
fn unrelated_publisher_key_refuses_before_fetching_the_archive() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use minisign_verify::PublicKey;

    let encoded = PUBLIC.lines().nth(1).unwrap();
    let mut raw = STANDARD.decode(encoded).unwrap();
    raw[2] ^= 1;
    let unrelated_key = PublicKey::from_base64(&STANDARD.encode(raw)).unwrap();
    let release = available();
    let source = responses(&release, PAYLOAD.to_vec(), SIGNATURE.to_vec());
    let parent = tempfile::tempdir().unwrap();
    assert!(matches!(
        download(
            &source,
            &unrelated_key,
            &release,
            parent.path(),
            &mut |_| true
        ),
        Err(UpdateError::Signature)
    ));
    assert_eq!(source.0.borrow().len(), 1);
    no_staging(parent.path());
}
