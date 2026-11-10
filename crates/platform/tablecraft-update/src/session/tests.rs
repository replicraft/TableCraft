use super::{Session, State, UpdateCommand, UpdateEvent, UpdateTaskError, classify_checked};
use crate::{
    UpdateError,
    installation::{InstallPlan, ManualReason},
    platform::Platform,
    release::{self, AvailableUpdate},
};
use semver::Version;
use serde_json::json;

fn available() -> AvailableUpdate {
    let metadata = json!({
        "tag_name": "v0.1.1",
        "draft": false,
        "prerelease": false,
        "html_url": "https://github.com/replicraft/TableCraft/releases/tag/v0.1.1",
        "assets": [
            {
                "name": "tablecraft-desktop-0.1.1-macos-arm64.zip",
                "size": 24,
                "browser_download_url": "https://github.com/replicraft/TableCraft/releases/download/v0.1.1/tablecraft-desktop-0.1.1-macos-arm64.zip",
            },
            {
                "name": "tablecraft-desktop-0.1.1-macos-arm64.zip.minisig",
                "size": 301,
                "browser_download_url": "https://github.com/replicraft/TableCraft/releases/download/v0.1.1/tablecraft-desktop-0.1.1-macos-arm64.zip.minisig",
            },
        ],
    });
    release::select(
        &serde_json::to_vec(&metadata).unwrap(),
        &Version::new(0, 1, 0),
        Platform::MacosArm64,
    )
    .unwrap()
    .unwrap()
}

fn session() -> Session {
    Session::new(Version::new(0, 1, 0), Platform::MacosArm64).unwrap()
}

#[test]
fn download_and_install_cannot_skip_check_or_preparation() {
    let mut session = session();
    for command in [UpdateCommand::Download, UpdateCommand::Install] {
        let mut events = Vec::new();
        assert!(matches!(
            session.execute(command, &|| false, &mut |event| {
                events.push(event);
                true
            }),
            Err(UpdateTaskError::InvalidState)
        ));
        assert!(events.is_empty());
        assert!(matches!(session.state, State::Idle));
    }
}

#[test]
fn cancelled_or_disconnected_check_stops_before_any_network_request() {
    let mut session = session();
    let mut events = Vec::new();
    let result = session.execute(UpdateCommand::Check, &|| true, &mut |event| {
        events.push(event);
        true
    });
    assert!(matches!(
        result,
        Err(UpdateTaskError::Operation(UpdateError::Cancelled))
    ));
    assert!(events.is_empty());
    assert!(matches!(session.state, State::Idle));

    let result = session.execute(UpdateCommand::Check, &|| false, &mut |event| {
        assert!(matches!(event, UpdateEvent::Checking));
        false
    });
    assert!(matches!(
        result,
        Err(UpdateTaskError::Operation(UpdateError::Cancelled))
    ));
    assert!(matches!(session.state, State::Idle));
}

#[test]
fn absent_release_never_hashes_or_classifies_the_local_installation() {
    let checked = classify_checked(None, || {
        panic!("no release must not inspect the installed package")
    })
    .unwrap();
    let mut session = session();
    let mut events = Vec::new();
    session
        .accept_checked(checked, &|| false, &mut |event| {
            events.push(event);
            true
        })
        .unwrap();
    assert!(matches!(events.as_slice(), [UpdateEvent::Current]));
    assert!(matches!(session.state, State::Idle));
}

#[test]
fn manual_installations_report_release_version_without_granting_download_or_install() {
    for reason in [
        ManualReason::DevelopmentBuild,
        ManualReason::MissingManifest,
        ManualReason::InvalidManifest,
        ManualReason::UnsupportedLocation,
    ] {
        let checked =
            classify_checked(Some(available()), || Ok(InstallPlan::Manual(reason))).unwrap();
        let mut session = session();
        let mut events = Vec::new();
        session
            .accept_checked(checked, &|| false, &mut |event| {
                events.push(event);
                true
            })
            .unwrap();
        assert!(
            matches!(events.as_slice(), [UpdateEvent::Manual { version: Some(version), reason: actual }] if version == "0.1.1" && *actual == reason)
        );
        assert!(matches!(
            session.execute(UpdateCommand::Download, &|| false, &mut |_| panic!(
                "manual installation must not download"
            )),
            Err(UpdateTaskError::InvalidState)
        ));
        assert!(matches!(
            session.execute(UpdateCommand::Install, &|| false, &mut |_| panic!(
                "manual installation must not install"
            )),
            Err(UpdateTaskError::InvalidState)
        ));
    }
}

#[test]
fn local_installation_errors_remain_errors_instead_of_manual_or_current() {
    let result = classify_checked(Some(available()), || Err(UpdateError::PayloadChanged));
    assert!(matches!(result, Err(UpdateError::PayloadChanged)));
}

#[test]
fn disconnected_manual_result_does_not_retain_a_download_capability() {
    let checked = classify_checked(Some(available()), || {
        Ok(InstallPlan::Manual(ManualReason::DevelopmentBuild))
    })
    .unwrap();
    let mut session = session();
    assert!(matches!(
        session.accept_checked(checked, &|| false, &mut |_| false),
        Err(UpdateTaskError::Operation(UpdateError::Cancelled))
    ));
    assert!(matches!(session.state, State::Idle));
    assert!(matches!(
        session.execute(UpdateCommand::Download, &|| false, &mut |_| panic!(
            "closed result must not start a download"
        )),
        Err(UpdateTaskError::InvalidState)
    ));
}
