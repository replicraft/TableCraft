use super::{cabinet, native, report::Report};
use crate::{UpdateError, platform::Platform};
use semver::Version;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Cursor, Write},
};

fn report() -> Value {
    json!({
        "template":"x64;1033",
        "properties":[
            {"name":"ProductName","value":"TableCraft"},
            {"name":"Manufacturer","value":"Replicraft"},
            {"name":"ProductVersion","value":"0.1.1"},
            {"name":"ALLUSERS","value":"1"},
            {"name":"UpgradeCode","value":"{F78632A2-14B8-5F28-B29D-760BDED29A9A}"},
            {"name":"ProductCode","value":"{00000000-0000-4000-8000-000000000001}"}
        ],
        "files":[
            {"id":"TableCraftDesktop","component":"DesktopApplication","name":"tablecraft-desktop.exe","size":"3","sequence":"1"},
            {"id":"ProjectLicenseFile","component":"ProjectLicense","name":"LICENSE","size":"3","sequence":"2"},
            {"id":"ThirdPartyNoticesFile","component":"ThirdPartyNotices","name":"ThirdPartyLicenses.txt","size":"3","sequence":"3"},
            {"id":"ReleaseManifestFile","component":"ReleaseManifest","name":"RELEASE.json","size":"3","sequence":"4"}
        ],
        "components":[
            {"id":"DesktopApplication","guid":"00000000-0000-4000-8000-000000000001","directory":"INSTALLFOLDER","attributes":"256","keypath":"TableCraftDesktop"},
            {"id":"ProjectLicense","guid":"00000000-0000-4000-8000-000000000002","directory":"INSTALLFOLDER","attributes":"256","keypath":"ProjectLicenseFile"},
            {"id":"ThirdPartyNotices","guid":"00000000-0000-4000-8000-000000000003","directory":"INSTALLFOLDER","attributes":"256","keypath":"ThirdPartyNoticesFile"},
            {"id":"ReleaseManifest","guid":"00000000-0000-4000-8000-000000000004","directory":"INSTALLFOLDER","attributes":"256","keypath":"ReleaseManifestFile"},
            {"id":"ApplicationRegistration","guid":"00000000-0000-4000-8000-000000000005","directory":"INSTALLFOLDER","attributes":"260","keypath":"RegistrationValue00"}
        ],
        "upgrades":[
            {"code":"F78632A2-14B8-5F28-B29D-760BDED29A9A","action":"WIX_UPGRADE_DETECTED"},
            {"code":"F78632A2-14B8-5F28-B29D-760BDED29A9A","action":"WIX_DOWNGRADE_DETECTED"}
        ],
        "custom_actions":[], "storages":[],
        "media":[{"disk":"1","last_sequence":"4","cabinet":"#cab1.cab"}]
    })
}

fn parse(value: Value) -> Report {
    serde_json::from_value(value).unwrap()
}

#[test]
fn native_table_policy_admits_both_target_templates_and_rejects_wrong_ownership() {
    let version = Version::parse("0.1.1").unwrap();
    assert!(
        parse(report())
            .validate(&version, Platform::WindowsX64)
            .is_ok()
    );
    let mut arm = report();
    arm["template"] = json!("Arm64;1033");
    assert!(
        parse(arm)
            .validate(&version, Platform::WindowsArm64)
            .is_ok()
    );
    for pointer in [
        "/components/0/attributes",
        "/components/0/keypath",
        "/upgrades/0/code",
        "/files/0/name",
    ] {
        let mut changed = report();
        *changed.pointer_mut(pointer).unwrap() = json!("unrelated");
        assert!(
            parse(changed)
                .validate(&version, Platform::WindowsX64)
                .is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn custom_actions_transforms_external_cabinets_and_duplicate_ids_refuse() {
    let version = Version::parse("0.1.1").unwrap();
    for field in ["custom_actions", "storages"] {
        let mut changed = report();
        changed[field] = json!([{"name":"execute-or-transform"}]);
        assert!(
            parse(changed)
                .validate(&version, Platform::WindowsX64)
                .is_err()
        );
    }
    let mut changed = report();
    changed["media"][0]["cabinet"] = json!("external.cab");
    assert!(
        parse(changed)
            .validate(&version, Platform::WindowsX64)
            .is_err()
    );
    let mut changed = report();
    changed["files"][1]["id"] = json!("TableCraftDesktop");
    assert!(
        parse(changed)
            .validate(&version, Platform::WindowsX64)
            .is_err()
    );
}

fn cabinet_bytes(names: &[&str]) -> Vec<u8> {
    let mut builder = cab::CabinetBuilder::new();
    let folder = builder.add_folder(cab::CompressionType::MsZip);
    for name in names {
        folder.add_file(*name);
    }
    let mut writer = builder.build(Cursor::new(Vec::new())).unwrap();
    while let Some(mut file) = writer.next_file().unwrap() {
        file.write_all(b"abc").unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn cabinet_ids_are_mapped_to_fixed_filenames_and_unlisted_or_repeated_members_refuse() {
    let names = [
        "TableCraftDesktop",
        "ProjectLicenseFile",
        "ThirdPartyNoticesFile",
        "ReleaseManifestFile",
    ];
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("payload.cab");
    fs::write(&source, cabinet_bytes(&names)).unwrap();
    let output = directory.path().join("extracted");
    cabinet::extract(&source, &output, &parse(report()), &|| false).unwrap();
    assert_eq!(
        fs::read(output.join("tablecraft-desktop.exe")).unwrap(),
        b"abc"
    );
    assert_eq!(fs::read_dir(output).unwrap().count(), 4);

    for final_name in ["../outside", "TableCraftDesktop", "unlisted"] {
        fs::write(
            &source,
            cabinet_bytes(&[names[0], names[1], names[2], final_name]),
        )
        .unwrap();
        let output = directory.path().join("refused");
        assert!(cabinet::extract(&source, &output, &parse(report()), &|| false).is_err());
        assert!(!output.exists());
    }
}

#[test]
fn cabinet_cancellation_and_mismatched_lengths_do_not_publish_payloads() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("payload.cab");
    fs::write(
        &source,
        cabinet_bytes(&[
            "TableCraftDesktop",
            "ProjectLicenseFile",
            "ThirdPartyNoticesFile",
            "ReleaseManifestFile",
        ]),
    )
    .unwrap();
    assert!(matches!(
        cabinet::extract(
            &source,
            &directory.path().join("cancelled"),
            &parse(report()),
            &|| true
        ),
        Err(UpdateError::Cancelled)
    ));
    let mut changed = report();
    changed["files"][0]["size"] = json!("536870913");
    assert!(
        cabinet::extract(
            &source,
            &directory.path().join("wrong-size"),
            &parse(changed),
            &|| false
        )
        .is_err()
    );
    assert!(!directory.path().join("wrong-size").exists());
}

#[test]
fn powershell_source_is_fixed_and_signing_secrets_are_scrubbed() {
    let command = native::command(
        std::path::Path::new("powershell.exe"),
        "fixed first-party source",
    );
    assert_eq!(command.get_args().count(), 5);
    for key in [
        "TABLECRAFT_UPDATE_SIGNING_KEY",
        "TABLECRAFT_UPDATE_SIGNING_PASSWORD",
    ] {
        assert!(
            command
                .get_envs()
                .any(|(name, value)| name == key && value.is_none())
        );
    }
}

#[cfg(windows)]
#[test]
#[ignore = "requires a built native MSI on a disposable Windows runner"]
fn native_generated_msi_is_inertly_inspected_and_decoded() {
    let path = std::path::PathBuf::from(
        std::env::var_os("TABLECRAFT_TEST_MSI_PATH").expect("explicit native fixture path"),
    );
    let version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    let platform = Platform::detect().unwrap();
    let report: Report = serde_json::from_value(
        native::inspect(&json!({"operation":"inspect", "path":path}), &|| false).unwrap(),
    )
    .unwrap();
    report.validate(&version, platform).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let cabinet_path = directory.path().join("payload.cab");
    native::inspect(
        &json!({"operation":"cabinet", "path":path, "destination":cabinet_path}),
        &|| false,
    )
    .unwrap();
    let output = directory.path().join("payload");
    cabinet::extract(&cabinet_path, &output, &report, &|| false).unwrap();
    super::preparation::validate_payload(&output, &report, &version, platform, &|| false).unwrap();
}

#[cfg(windows)]
#[test]
#[ignore = "requires an installed MSI on a disposable Windows runner"]
fn native_registered_installation_binds_the_actual_component_and_payload() {
    let executable = std::path::PathBuf::from(
        std::env::var_os("TABLECRAFT_TEST_INSTALLED_EXE").expect("explicit installed fixture path"),
    )
    .canonicalize()
    .unwrap();
    let report = native::inspect(&json!({"operation":"detect"}), &|| false).unwrap();
    let products = serde_json::from_value(report).unwrap();
    let version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    let installed = super::installed::seal_registered(
        executable,
        &version,
        Platform::detect().unwrap(),
        products,
        &|| false,
    )
    .unwrap();
    assert_eq!(installed.version(), &version);
}
