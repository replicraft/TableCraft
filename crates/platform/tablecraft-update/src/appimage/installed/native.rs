//! Explicit native qualification only; ordinary tests never spawn a relaunch helper.

use super::authenticate_at;
use crate::{
    UpdateError,
    appimage::context::Context,
    artifact::{self, ArtifactKind},
    download,
    http::{Body, Transport},
    package,
    platform::Platform,
    release,
    target::UpdateTarget,
    trust,
};
use semver::Version;
use serde::Deserialize;
use serde_json::json;
use std::{
    env,
    fs::{self, File},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

#[derive(Deserialize)]
struct Inputs {
    current: String,
    next: String,
    target: String,
    qualification_receiver: String,
}

struct Source<'a> {
    root: &'a Path,
    current: &'a Version,
    next: &'a Version,
    platform: Platform,
}

impl Transport for Source<'_> {
    fn get(&self, url: &str) -> Result<Body, UpdateError> {
        let old = ArtifactKind::AppImage.file_name(self.current, self.platform)?;
        let new = ArtifactKind::AppImage.file_name(self.next, self.platform)?;
        let name = if url == artifact::download_url(self.current, &format!("{old}.minisig")) {
            "original.AppImage.minisig"
        } else if url == artifact::download_url(self.next, &format!("{new}.minisig")) {
            "candidate.AppImage.minisig"
        } else if url == artifact::download_url(self.next, &new) {
            "candidate.AppImage"
        } else {
            return Err(UpdateError::UnauthorizedUrl);
        };
        let reader = File::open(self.root.join(name)).map_err(UpdateError::Io)?;
        let length = reader.metadata().map_err(UpdateError::Io)?.len();
        Ok(Body {
            reader: Box::new(reader),
            length: Some(length),
        })
    }
}

#[test]
#[ignore = "requires native Linux AppImage qualification fixtures and headless receiver"]
fn native_actual_image_and_headless_receiver_wait_for_normal_parent_exit() {
    let platform = Platform::detect().expect("native supported target");
    assert!(
        matches!(platform, Platform::LinuxX64 | Platform::LinuxArm64),
        "qualification requires native Linux"
    );
    let root = PathBuf::from(
        env::var_os("TABLECRAFT_TEST_APPIMAGE_ROOT").expect("explicit isolated fixture root"),
    );
    let inputs: Inputs =
        serde_json::from_slice(&fs::read(root.join("inputs.json")).unwrap()).unwrap();
    let current: Version = inputs.current.parse().unwrap();
    let next: Version = inputs.next.parse().unwrap();
    assert_eq!(inputs.target, platform.target());
    assert_eq!(
        inputs.qualification_receiver,
        "tablecraft-headless-update-receiver"
    );
    let key_text = fs::read_to_string(root.join("TEST-ONLY.pub")).unwrap();
    assert!(key_text.lines().next().unwrap().contains("TEST ONLY"));
    let key = trust::parse_key(&key_text).unwrap();
    let appdir = root.join("original-runtime/squashfs-root");
    let original = root.join("original.AppImage");
    let executable = appdir.join("usr/bin/tablecraft-desktop");
    let source = Source {
        root: &root,
        current: &current,
        next: &next,
        platform,
    };
    let installed = authenticate_at(
        &source,
        &key,
        Context {
            image: original.clone(),
            appdir,
        },
        &executable,
        &current,
        platform,
        &|| false,
    )
    .unwrap();
    let original_hash = package::hash_file(&original, release::MAX_ASSET_BYTES, &|| false)
        .unwrap()
        .unwrap();
    let name = ArtifactKind::AppImage.file_name(&next, platform).unwrap();
    let url = artifact::download_url(&next, &name);
    let metadata = json!({
        "tag_name": format!("v{}", next), "draft": false, "prerelease": !next.pre.is_empty(),
        "html_url": format!("https://github.com/replicraft/TableCraft/releases/tag/v{}", next),
        "assets": [
            { "name": name, "size": fs::metadata(root.join("candidate.AppImage")).unwrap().len(), "browser_download_url": url },
            { "name": format!("{name}.minisig"), "size": fs::metadata(root.join("candidate.AppImage.minisig")).unwrap().len(), "browser_download_url": format!("{url}.minisig") }
        ]
    });
    let available = release::select(
        &serde_json::to_vec(&metadata).unwrap(),
        &current,
        platform,
        ArtifactKind::AppImage,
    )
    .unwrap()
    .unwrap();
    let verified = download::download(
        &source,
        &key,
        &available,
        installed.staging_parent(),
        &mut |_| true,
    )
    .unwrap();
    let prepared = verified
        .prepare(UpdateTarget::AppImage(installed), &|| false)
        .unwrap();
    let receipt = root.join("receiver.txt");
    assert!(!receipt.exists());
    let applied = prepared.apply(&|| false).unwrap();
    let helper = applied
        .helper_process_id()
        .expect("actual detached native helper");

    // A live native parent must delay relaunch and retain the rollback original.
    thread::sleep(Duration::from_millis(1500));
    assert!(
        !receipt.exists(),
        "helper launched before its parent exited"
    );
    let backup = applied.retained_directory();
    assert_eq!(
        package::hash_file(
            &backup.join("original.AppImage"),
            release::MAX_ASSET_BYTES,
            &|| false
        )
        .unwrap(),
        Some(original_hash)
    );
    assert_eq!(
        fs::read(root.join("private.accdb")).unwrap(),
        b"untouched qualification database"
    );
    let handoff = json!({
        "helper_pid": helper, "parent_pid": std::process::id(), "backup": backup,
        "original_sha256": package::hash_text(&original_hash), "waited_while_parent_alive": true,
        "qualification": "actual-image-source-private-proof-and-real-helper-headless-receiver",
        "gui_relaunch_qualified": false, "running_desktop_context_qualified": false
    });
    let mut report = File::create(root.join("handoff.json")).unwrap();
    serde_json::to_writer_pretty(&mut report, &handoff).unwrap();
    report.sync_all().unwrap();
    // Returning ends the test process normally. Child Drop does not kill the
    // detached helper; the outer harness checks its eventual receiver/backup state.
}
