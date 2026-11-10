use super::{AvailableUpdate, select};
use crate::artifact::ArtifactKind;
use crate::{UpdateError, platform::Platform};
use semver::Version;
use serde_json::{Value, json};

fn release(version: &str) -> Value {
    let archive = format!("tablecraft-desktop-{version}-macos-arm64.zip");
    let download =
        format!("https://github.com/replicraft/TableCraft/releases/download/v{version}/");

    json!({
        "tag_name": format!("v{version}"),
        "draft": false,
        "prerelease": version.contains('-'),
        "html_url": format!("https://github.com/replicraft/TableCraft/releases/tag/v{version}"),
        "assets": [
            {
                "name": archive,
                "size": 24,
                "browser_download_url": format!("{download}{archive}"),
            },
            {
                "name": format!("{archive}.minisig"),
                "size": 301,
                "browser_download_url": format!("{download}{archive}.minisig"),
            },
            {
                "name": "unrelated-notices.txt",
                "size": 123,
                "browser_download_url": "https://example.invalid/not-selected",
            },
        ],
        "unknown_future_field": {"nested": true},
    })
}

fn selected(value: &Value, current: &str) -> Result<Option<AvailableUpdate>, UpdateError> {
    select(
        &serde_json::to_vec(value).unwrap(),
        &Version::parse(current).unwrap(),
        Platform::MacosArm64,
        ArtifactKind::PortableArchive,
    )
}

#[test]
fn selects_exact_newer_release_and_both_download_identities() {
    let update = selected(&release("0.2.0"), "0.1.0").unwrap().unwrap();
    assert_eq!(update.version(), &Version::parse("0.2.0").unwrap());
    assert_eq!(update.platform(), Platform::MacosArm64);
    assert_eq!(
        update.asset_name(),
        "tablecraft-desktop-0.2.0-macos-arm64.zip"
    );
    assert_eq!(update.asset_bytes(), 24);
    assert_eq!(update.signature_bytes(), 301);
    assert_eq!(
        update.asset_url(),
        "https://github.com/replicraft/TableCraft/releases/download/v0.2.0/tablecraft-desktop-0.2.0-macos-arm64.zip"
    );
    assert_eq!(
        update.signature_url(),
        "https://github.com/replicraft/TableCraft/releases/download/v0.2.0/tablecraft-desktop-0.2.0-macos-arm64.zip.minisig"
    );
    assert_eq!(
        update.release_url(),
        "https://github.com/replicraft/TableCraft/releases/tag/v0.2.0"
    );
}

#[test]
fn rollback_equal_versions_and_build_only_changes_are_not_updates() {
    for (available, installed) in [
        ("0.1.0", "0.1.1"),
        ("0.1.0", "0.1.0"),
        ("0.1.0+new", "0.1.0+old"),
        ("0.1.0+new", "0.1.0"),
        ("0.1.0", "0.1.0+old"),
    ] {
        assert!(
            selected(&release(available), installed).unwrap().is_none(),
            "{available} must not update {installed}"
        );
    }
}

#[test]
fn stable_builds_refuse_prereleases_but_prerelease_builds_follow_precedence() {
    assert!(
        selected(&release("0.2.0-beta.1"), "0.1.0")
            .unwrap()
            .is_none()
    );
    assert!(
        selected(&release("0.2.0-beta.2"), "0.2.0-beta.1")
            .unwrap()
            .is_some()
    );
    assert!(
        selected(&release("0.2.0-beta.1"), "0.2.0-beta.2")
            .unwrap()
            .is_none()
    );
    assert!(
        selected(&release("0.2.0"), "0.2.0-beta.1")
            .unwrap()
            .is_some()
    );
}

#[test]
fn invalid_tags_drafts_and_inconsistent_prerelease_flags_refuse() {
    for tag in ["0.2.0", "v02.0.0", "v0.2", "v0.2.0/other", "v0.2.0\n"] {
        let mut value = release("0.2.0");
        value["tag_name"] = json!(tag);
        assert!(matches!(
            selected(&value, "0.1.0"),
            Err(UpdateError::ReleaseMetadata)
        ));
    }

    let mut draft = release("0.2.0");
    draft["draft"] = json!(true);
    assert!(matches!(
        selected(&draft, "0.1.0"),
        Err(UpdateError::ReleaseMetadata)
    ));

    for version in ["0.2.0", "0.2.0-beta.1"] {
        let mut value = release(version);
        value["prerelease"] = json!(!version.contains('-'));
        assert!(matches!(
            selected(&value, "0.1.0-beta.1"),
            Err(UpdateError::ReleaseMetadata)
        ));
    }
}

#[test]
fn selected_asset_and_release_urls_cannot_redirect_repository_or_version() {
    let bad_urls = [
        "http://github.com/replicraft/TableCraft/releases/download/v0.2.0/payload.zip",
        "https://example.invalid/payload.zip",
        "https://github.com/other/TableCraft/releases/download/v0.2.0/payload.zip",
        "https://github.com/replicraft/TableCraft/releases/download/v0.1.0/payload.zip",
        "https://github.com/replicraft/TableCraft/releases/download/v0.2.0/payload.zip?token=1",
    ];

    for url in bad_urls {
        for index in [0, 1] {
            let mut value = release("0.2.0");
            value["assets"][index]["browser_download_url"] = json!(url);
            assert!(matches!(
                selected(&value, "0.1.0"),
                Err(UpdateError::ReleaseMetadata)
            ));
        }
    }

    let mut value = release("0.2.0");
    value["html_url"] = json!("https://github.com/other/TableCraft/releases/tag/v0.2.0");
    assert!(matches!(
        selected(&value, "0.1.0"),
        Err(UpdateError::ReleaseMetadata)
    ));
}

#[test]
fn missing_duplicate_and_cross_platform_assets_refuse() {
    for index in [0, 1] {
        let mut missing = release("0.2.0");
        missing["assets"].as_array_mut().unwrap().remove(index);
        assert!(matches!(
            selected(&missing, "0.1.0"),
            Err(UpdateError::ReleaseMetadata)
        ));

        let mut duplicate = release("0.2.0");
        let copied = duplicate["assets"][index].clone();
        duplicate["assets"].as_array_mut().unwrap().push(copied);
        assert!(matches!(
            selected(&duplicate, "0.1.0"),
            Err(UpdateError::ReleaseMetadata)
        ));
    }

    let value = serde_json::to_vec(&release("0.2.0")).unwrap();
    assert!(matches!(
        select(
            &value,
            &Version::parse("0.1.0").unwrap(),
            Platform::WindowsX64,
            ArtifactKind::PortableArchive,
        ),
        Err(UpdateError::ReleaseMetadata)
    ));
}

#[test]
fn asset_lengths_are_positive_bounded_and_unsigned() {
    for (index, sizes) in [(0, [0, 512 * 1024 * 1024 + 1]), (1, [0, 4097])] {
        for size in sizes {
            let mut value = release("0.2.0");
            value["assets"][index]["size"] = json!(size);
            assert!(matches!(
                selected(&value, "0.1.0"),
                Err(UpdateError::AssetTooLarge)
            ));
        }
    }

    for invalid in [json!(-1), json!(1.5), json!("24")] {
        let mut value = release("0.2.0");
        value["assets"][0]["size"] = invalid;
        assert!(matches!(
            selected(&value, "0.1.0"),
            Err(UpdateError::ReleaseMetadata)
        ));
    }
}

#[test]
fn metadata_limits_and_duplicate_known_json_fields_refuse() {
    let oversized = vec![b' '; 1024 * 1024 + 1];
    assert!(matches!(
        select(
            &oversized,
            &Version::parse("0.1.0").unwrap(),
            Platform::MacosArm64,
            ArtifactKind::PortableArchive,
        ),
        Err(UpdateError::ReleaseTooLarge)
    ));

    for bytes in [
        b"not JSON".as_slice(),
        br#"{"tag_name":"v0.2.0","tag_name":"v0.3.0","draft":false,"prerelease":false,"html_url":"ignored","assets":[]}"#.as_slice(),
    ] {
        assert!(matches!(
            select(bytes, &Version::parse("0.1.0").unwrap(), Platform::MacosArm64, ArtifactKind::PortableArchive),
            Err(UpdateError::ReleaseMetadata)
        ));
    }
}

#[test]
fn installer_selection_requires_its_exact_payload_and_signature_pair() {
    for (kind, platform, name) in [
        (
            ArtifactKind::AppImage,
            Platform::LinuxX64,
            "tablecraft-desktop-0.2.0-linux-x64.AppImage",
        ),
        (
            ArtifactKind::WindowsMsi,
            Platform::WindowsArm64,
            "tablecraft-desktop-0.2.0-windows-arm64.msi",
        ),
    ] {
        let mut metadata = release("0.2.0");
        metadata["assets"][0]["name"] = json!(name);
        metadata["assets"][0]["browser_download_url"] = json!(format!(
            "https://github.com/replicraft/TableCraft/releases/download/v0.2.0/{name}"
        ));
        metadata["assets"][1]["name"] = json!(format!("{name}.minisig"));
        metadata["assets"][1]["browser_download_url"] = json!(format!(
            "https://github.com/replicraft/TableCraft/releases/download/v0.2.0/{name}.minisig"
        ));
        let bytes = serde_json::to_vec(&metadata).unwrap();
        let available = select(&bytes, &Version::new(0, 1, 0), platform, kind)
            .unwrap()
            .unwrap();
        assert_eq!(available.kind(), kind);
        assert_eq!(available.asset_name(), name);
        assert!(matches!(
            select(
                &bytes,
                &Version::new(0, 1, 0),
                platform,
                ArtifactKind::PortableArchive
            ),
            Err(UpdateError::ReleaseMetadata)
        ));
    }
}
