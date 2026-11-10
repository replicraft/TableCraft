use super::{UpdateFailure, UpdateManualReason, UpdateStatus};
use crate::{Action, Locale, Localizer, State, Theme, workspace::backstage::Backstage};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn harness(
    locale: Locale,
    theme: Theme,
    size: egui::Vec2,
) -> Harness<'static, (State, Vec<Action>)> {
    let state = State {
        backstage: Backstage::About,
        localizer: Localizer::new(locale).unwrap(),
        theme,
        path: "untouched database.accdb".into(),
        password: "untouched open password".into(),
        ..State::default()
    };
    let mut harness = Harness::builder().with_size(size).build_ui_state(
        |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
        (state, Vec::new()),
    );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
}

#[test]
fn actual_about_buttons_dispatch_once_and_cancel_waits_for_host_acknowledgment() {
    let mut harness = harness(Locale::EnUs, Theme::Light, egui::vec2(1280.0, 820.0));
    harness.state_mut().0.sql = "SELECT 'untouched SQL draft'".into();
    harness.state_mut().0.set_updates_available(true);
    harness.step();
    let check = harness.get_by_role_and_label(Role::Button, "Check for updates");
    check.click_accesskit();
    check.click_accesskit();
    harness.run_steps(2);
    assert!(matches!(
        harness.state().0.update_status(),
        UpdateStatus::Checking
    ));
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::CheckForUpdates]
    ));

    harness
        .get_by_role_and_label(Role::Button, "Cancel update")
        .click_accesskit();
    harness.run_steps(2);
    assert!(matches!(
        harness.state().0.update_status(),
        UpdateStatus::Cancelling
    ));
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::CheckForUpdates, Action::CancelUpdate]
    ));

    harness.state_mut().0.set_update_status(UpdateStatus::Idle);
    harness.step();
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Check for updates")
            .accesskit_node()
            .is_disabled()
    );
    assert_eq!(harness.state().0.sql, "SELECT 'untouched SQL draft'");
    assert_eq!(harness.state().0.path, "untouched database.accdb");
    assert_eq!(harness.state().0.password, "untouched open password");
}

#[test]
fn download_and_install_require_their_actual_phases_and_explicit_host_install_permission() {
    let mut harness = harness(Locale::EnUs, Theme::Dark, egui::vec2(900.0, 580.0));
    let state = &mut harness.state_mut().0;
    state.set_updates_available(true);
    state.set_update_status(UpdateStatus::Available {
        version: "0.1.1".into(),
    });
    harness.step();
    harness
        .get_by_role_and_label(Role::Button, "Download update")
        .click_accesskit();
    harness.run_steps(2);
    assert!(matches!(
        harness.state().0.update_status(),
        UpdateStatus::Downloading {
            received: 0,
            total: 0
        }
    ));
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::DownloadUpdate]
    ));

    harness
        .state_mut()
        .0
        .set_update_status(UpdateStatus::Ready {
            version: "0.1.1".into(),
        });
    harness.step();
    let install = harness.get_by_role_and_label(Role::Button, "Restart and update");
    assert!(install.accesskit_node().is_disabled());
    install.click_accesskit();
    harness.step();
    assert!(matches!(
        harness.state().0.update_status(),
        UpdateStatus::Ready { .. }
    ));
    assert_eq!(harness.state().1.len(), 1);

    harness.state_mut().0.set_update_can_install(true);
    harness.step();
    let install = harness.get_by_role_and_label(Role::Button, "Restart and update");
    install.click_accesskit();
    install.click_accesskit();
    harness.run_steps(2);
    assert!(matches!(
        harness.state().0.update_status(),
        UpdateStatus::Installing
    ));
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::DownloadUpdate, Action::InstallUpdate]
    ));
}

#[test]
fn synthetic_and_unconnected_hosts_keep_update_controls_visible_but_inert() {
    for synthetic in [false, true] {
        for (status, button) in [
            (UpdateStatus::Idle, "Check for updates"),
            (
                UpdateStatus::Available {
                    version: "0.1.1".into(),
                },
                "Download update",
            ),
            (
                UpdateStatus::Ready {
                    version: "0.1.1".into(),
                },
                "Restart and update",
            ),
        ] {
            let mut harness = harness(Locale::EnUs, Theme::Light, egui::vec2(1280.0, 820.0));
            let state = &mut harness.state_mut().0;
            state.mock_preview = synthetic;
            state.set_updates_available(synthetic);
            state.set_update_can_install(true);
            state.set_update_status(status);
            harness.step();
            let command = harness.get_by_role_and_label(Role::Button, button);
            assert!(command.accesskit_node().is_disabled());
            command.click_accesskit();
            harness.step();
            assert!(harness.state().1.is_empty());
        }
    }
}

#[test]
fn every_supported_locale_renders_statuses_without_fallback_or_backend_actions() {
    for locale in Locale::ALL {
        for theme in [Theme::Light, Theme::Dark] {
            let mut harness = harness(locale, theme, egui::vec2(900.0, 580.0));
            for status in [
                UpdateStatus::Current,
                UpdateStatus::Available {
                    version: "0.1.1".into(),
                },
                UpdateStatus::Downloading {
                    received: 1024,
                    total: 4096,
                },
                UpdateStatus::Ready {
                    version: "0.1.1".into(),
                },
                UpdateStatus::Manual {
                    version: Some("0.1.1".into()),
                    reason: UpdateManualReason::DevelopmentBuild,
                },
                UpdateStatus::Manual {
                    version: None,
                    reason: UpdateManualReason::ManagedInstallation,
                },
                UpdateStatus::Manual {
                    version: None,
                    reason: UpdateManualReason::UnrecognizedInstallation,
                },
                UpdateStatus::Manual {
                    version: None,
                    reason: UpdateManualReason::UnsupportedPlatform,
                },
                UpdateStatus::Failed(UpdateFailure::Network),
                UpdateStatus::Failed(UpdateFailure::Signature),
                UpdateStatus::Failed(UpdateFailure::Storage),
                UpdateStatus::Failed(UpdateFailure::InvalidRelease),
                UpdateStatus::Failed(UpdateFailure::InvalidArchive),
                UpdateStatus::Failed(UpdateFailure::Installation),
                UpdateStatus::Failed(UpdateFailure::NotConfigured),
                UpdateStatus::Failed(UpdateFailure::UnsupportedPlatform),
            ] {
                harness.state_mut().0.set_update_status(status);
                harness.run_steps(2);
                assert_eq!(
                    harness.state().0.localizer.diagnostics(),
                    Default::default(),
                    "{locale:?} {theme:?}"
                );
                assert!(harness.state().1.is_empty());
            }
        }
    }
}
