use super::extract;
use crate::{UpdateError, platform::Platform};
use flate2::{Compression, write::GzEncoder};
use semver::Version;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    fs::{self, File},
    io::{Cursor, Write},
    path::Path,
};
use tempfile::TempDir;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const PLIST: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
\t<key>CFBundleDisplayName</key>
\t<string>TableCraft</string>
\t<key>CFBundleExecutable</key>
\t<string>tablecraft-desktop</string>
\t<key>CFBundleIconFile</key>
\t<string>TableCraft.icns</string>
\t<key>CFBundleIdentifier</key>
\t<string>org.replicraft.tablecraft</string>
\t<key>CFBundleName</key>
\t<string>TableCraft</string>
\t<key>CFBundlePackageType</key>
\t<string>APPL</string>
\t<key>CFBundleShortVersionString</key>
\t<string>0.1.0</string>
\t<key>CFBundleVersion</key>
\t<string>0.1.0</string>
\t<key>NSHighResolutionCapable</key>
\t<true/>
</dict>
</plist>
";

fn files(platform: Platform) -> Vec<(String, Vec<u8>)> {
    let mac = matches!(platform, Platform::MacosX64 | Platform::MacosArm64);
    let windows = matches!(platform, Platform::WindowsX64 | Platform::WindowsArm64);
    let executable = if mac {
        "TableCraft.app/Contents/MacOS/tablecraft-desktop"
    } else if windows {
        "tablecraft-desktop.exe"
    } else {
        "tablecraft-desktop"
    };
    let resource = if mac {
        "TableCraft.app/Contents/Resources/"
    } else {
        ""
    };
    let mut files = vec![
        (
            executable.to_owned(),
            b"independent executable carrier".to_vec(),
        ),
        (
            format!("{resource}LICENSE"),
            b"project license fixture".to_vec(),
        ),
        (
            format!("{resource}ThirdPartyLicenses.txt"),
            b"dependency notices fixture".to_vec(),
        ),
    ];
    if mac {
        files.push(("TableCraft.app/Contents/Info.plist".into(), PLIST.to_vec()));
        files.push((
            format!("{resource}TableCraft.icns"),
            b"independent icon carrier".to_vec(),
        ));
    }
    let hashes: serde_json::Map<String, serde_json::Value> = files
        .iter()
        .map(|(name, content)| {
            (
                name.clone(),
                json!(format!("{:x}", Sha256::digest(content))),
            )
        })
        .collect();
    files.push((
        format!("{resource}RELEASE.json"),
        serde_json::to_vec(&json!({
            "product": "tablecraft-desktop", "version": "0.1.0",
            "target": platform.target(), "files": hashes,
        }))
        .unwrap(),
    ));
    files
}

fn zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        let mode = if name.ends_with("tablecraft-desktop") || name.ends_with(".exe") {
            0o755
        } else {
            0o644
        };
        let options = SimpleFileOptions::default()
            .unix_permissions(mode)
            .compression_method(CompressionMethod::Deflated)
            .large_file(true);
        writer.start_file(name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn tar(files: &[(String, Vec<u8>)], symlink: bool) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, bytes) in files {
        let mut header = tar::Header::new_gnu();
        header.set_mode(if name == "tablecraft-desktop" {
            0o755
        } else {
            0o644
        });
        header.set_size(bytes.len() as u64);
        if symlink && name == "LICENSE" {
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_link_name("/outside/not-a-package").unwrap();
            header.set_size(0);
            header.set_cksum();
            builder.append_data(&mut header, name, &[][..]).unwrap();
        } else {
            header.set_cksum();
            builder
                .append_data(&mut header, name, bytes.as_slice())
                .unwrap();
        }
    }
    gzip(&builder.into_inner().unwrap())
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn source(root: &Path, bytes: &[u8]) -> File {
    let path = root.join("authenticated-input");
    fs::write(&path, bytes).unwrap();
    File::open(path).unwrap()
}

fn refused(bytes: &[u8], platform: Platform) {
    let parent = TempDir::new().unwrap();
    let result = extract(
        source(parent.path(), bytes),
        platform,
        &Version::new(0, 1, 0),
        parent.path(),
        &|| false,
    );
    assert!(matches!(result, Err(UpdateError::Archive)), "{result:?}");
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 1);
}

#[test]
fn all_six_platforms_extract_exact_payloads_and_cleanup_with_the_owner() {
    for platform in [
        Platform::WindowsX64,
        Platform::WindowsArm64,
        Platform::MacosX64,
        Platform::MacosArm64,
        Platform::LinuxX64,
        Platform::LinuxArm64,
    ] {
        let parent = TempDir::new().unwrap();
        let files = files(platform);
        let bytes = if matches!(platform, Platform::LinuxX64 | Platform::LinuxArm64) {
            tar(&files, false)
        } else {
            zip(&files)
        };
        let extracted = extract(
            source(parent.path(), &bytes),
            platform,
            &Version::new(0, 1, 0),
            parent.path(),
            &|| false,
        )
        .unwrap();
        let root = extracted.root().to_path_buf();
        for (name, expected) in files {
            assert_eq!(fs::read(root.join(name)).unwrap(), expected);
        }
        assert_eq!(
            fs::read(extracted.executable()).unwrap(),
            b"independent executable carrier"
        );
        assert_eq!(
            extracted.bundle().is_some(),
            matches!(platform, Platform::MacosX64 | Platform::MacosArm64)
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(extracted.executable())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o7777,
                0o755
            );
        }
        drop(extracted);
        assert!(!root.exists());
    }
}

#[test]
fn unlisted_traversal_absolute_alias_reserved_and_directory_paths_refuse() {
    for name in [
        "../LICENSE",
        "/LICENSE",
        "C:LICENSE",
        "a\\LICENSE",
        "CON",
        "LICENSE/",
        "./LICENSE",
    ] {
        let mut files = files(Platform::WindowsX64);
        files[1].0 = name.into();
        refused(&zip(&files), Platform::WindowsX64);
    }
}

#[test]
fn archive_metadata_entry_count_and_file_type_are_bounded_before_writes() {
    let original = zip(&files(Platform::WindowsX64));
    let end = original.len() - 22;
    let mut count = original.clone();
    count[end + 10..end + 12].copy_from_slice(&u16::MAX.to_le_bytes());
    refused(&count, Platform::WindowsX64);

    let central = original
        .windows(4)
        .position(|bytes| bytes == b"PK\x01\x02")
        .unwrap();
    for mode in [0o120777u32, 0o100777u32, 0o020644u32] {
        let mut changed = original.clone();
        changed[central + 38..central + 42].copy_from_slice(&(mode << 16).to_le_bytes());
        refused(&changed, Platform::WindowsX64);
    }
    let mut encrypted = original.clone();
    encrypted[central + 8] |= 1;
    refused(&encrypted, Platform::WindowsX64);
}

#[test]
fn missing_extra_and_duplicate_payload_members_refuse() {
    let mut missing = files(Platform::WindowsX64);
    missing.pop();
    refused(&zip(&missing), Platform::WindowsX64);
    let mut extra = files(Platform::WindowsX64);
    extra.push(("extra.dll".into(), b"not owned".to_vec()));
    refused(&zip(&extra), Platform::WindowsX64);

    // Write duplicate names through an independent central-directory mutation:
    // ThirdPartyLicenses.txt and a padded unknown name have equal lengths.
    let mut ordinary = files(Platform::WindowsX64);
    ordinary[1].0 = "ThirdPartyLicenses.txX".into();
    let mut bytes = zip(&ordinary);
    let renamed = b"ThirdPartyLicenses.txX";
    let mut occurrences = 0;
    for offset in 0..=bytes.len() - renamed.len() {
        if &bytes[offset..offset + renamed.len()] == renamed {
            bytes[offset + renamed.len() - 1] = b't';
            occurrences += 1;
        }
    }
    assert_eq!(occurrences, 2);
    refused(&bytes, Platform::WindowsX64);
}

#[test]
fn mismatched_payload_hash_and_release_identity_refuse_after_cleanup() {
    for field in ["product", "version", "target", "files"] {
        let mut files = files(Platform::WindowsX64);
        let mut manifest: serde_json::Value = serde_json::from_slice(&files[3].1).unwrap();
        manifest[field] = if field == "files" {
            json!({})
        } else {
            json!("wrong")
        };
        files[3].1 = serde_json::to_vec(&manifest).unwrap();
        refused(&zip(&files), Platform::WindowsX64);
    }
    let mut files = files(Platform::WindowsX64);
    files[0].1.push(b'!');
    refused(&zip(&files), Platform::WindowsX64);
}

#[test]
fn duplicate_manifest_keys_and_hash_members_refuse() {
    for replacement in ["{\"product\":\"tablecraft-desktop\",", "{\"files\":{},"] {
        let mut files = files(Platform::WindowsX64);
        let original = str::from_utf8(&files[3].1).unwrap();
        files[3].1 = original.replacen('{', replacement, 1).into_bytes();
        refused(&zip(&files), Platform::WindowsX64);
    }
    let mut files = files(Platform::WindowsX64);
    let original = str::from_utf8(&files[3].1).unwrap();
    files[3].1 = original
        .replacen("\"files\":{", "\"files\":{\"LICENSE\":\"duplicate\",", 1)
        .into_bytes();
    refused(&zip(&files), Platform::WindowsX64);
}

#[test]
fn macos_identity_cannot_be_changed_even_with_matching_manifest_hashes() {
    let mut files = files(Platform::MacosArm64);
    files[3].1 = str::from_utf8(PLIST)
        .unwrap()
        .replace("org.replicraft.tablecraft", "org.other.application")
        .into_bytes();
    let changed_hash = format!("{:x}", Sha256::digest(&files[3].1));
    let mut manifest: serde_json::Value = serde_json::from_slice(&files[5].1).unwrap();
    manifest["files"]["TableCraft.app/Contents/Info.plist"] = json!(changed_hash);
    files[5].1 = serde_json::to_vec(&manifest).unwrap();
    refused(&zip(&files), Platform::MacosArm64);
}

#[test]
fn linux_links_trailing_streams_and_bad_gzip_trailer_refuse() {
    let files = files(Platform::LinuxX64);
    refused(&tar(&files, true), Platform::LinuxX64);
    let original = tar(&files, false);
    let mut trailing = original.clone();
    trailing.extend_from_slice(b"unowned trailing bytes");
    refused(&trailing, Platform::LinuxX64);
    let mut concatenated = original.clone();
    concatenated.extend_from_slice(&gzip(b"another member"));
    refused(&concatenated, Platform::LinuxX64);
    refused(&original[..original.len() - 1], Platform::LinuxX64);
}

#[test]
fn cancellation_removes_partial_staging_without_touching_the_input() {
    let parent = TempDir::new().unwrap();
    let bytes = zip(&files(Platform::WindowsX64));
    let checks = Cell::new(0);
    let cancelled = || {
        checks.set(checks.get() + 1);
        checks.get() >= 5
    };
    let result = extract(
        source(parent.path(), &bytes),
        Platform::WindowsX64,
        &Version::new(0, 1, 0),
        parent.path(),
        &cancelled,
    );
    assert!(matches!(result, Err(UpdateError::Cancelled)));
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 1);
    assert_eq!(
        fs::read(parent.path().join("authenticated-input")).unwrap(),
        bytes
    );
}
