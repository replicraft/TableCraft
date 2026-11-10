//! Source-free update observations; this host never checks, downloads or installs.
use tablecraft_ui::{UpdateFailure, UpdateManualReason, UpdateStatus, preview::Preview};

const PREVIEW_VERSION: &str = "0.1.1";
const PREVIEW_TOTAL_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn is_mode(mode: &str) -> bool {
    matches!(
        mode,
        "none"
            | "idle"
            | "checking"
            | "available"
            | "downloading"
            | "ready"
            | "blocked"
            | "manual"
            | "failed"
    )
}

pub(super) fn is_animated(mode: &str) -> bool {
    mode == "checking"
}

pub(super) fn apply(preview: &mut Preview, mode: &str) {
    let state = preview.state_mut();
    // Model an otherwise clean browser; retained sample SQL is not a user draft
    // in this resource-free presentation scene. No host permissions are issued.
    state.sql.clear();
    state.set_updates_available(true);
    state.set_update_can_install(mode == "ready");
    let status = match mode {
        "idle" => UpdateStatus::Idle,
        "checking" => UpdateStatus::Checking,
        "available" => UpdateStatus::Available {
            version: PREVIEW_VERSION.into(),
        },
        "downloading" => UpdateStatus::Downloading {
            received: PREVIEW_TOTAL_BYTES / 4,
            total: PREVIEW_TOTAL_BYTES,
        },
        "ready" | "blocked" => UpdateStatus::Ready {
            version: PREVIEW_VERSION.into(),
        },
        "manual" => UpdateStatus::Manual {
            version: Some(PREVIEW_VERSION.into()),
            reason: UpdateManualReason::DevelopmentBuild,
        },
        "failed" => UpdateStatus::Failed(UpdateFailure::Signature),
        _ => return,
    };
    state.set_update_status(status);
}
