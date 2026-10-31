#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

#[path = "offline_sessions/faults.rs"]
mod faults;
#[path = "offline_sessions/lifecycle.rs"]
mod lifecycle;
#[path = "offline_sessions/model.rs"]
mod model;
#[path = "offline_sessions/refresh.rs"]
mod refresh;

use std::{
    fs,
    path::{Path, PathBuf},
};
use tablecraft_access::editing::offline::OfflineWriteSession;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn write_original(root: &Path) -> PathBuf {
    let database = root.join("target.mdb");
    fs::write(&database, model::original()).unwrap();
    database
}

fn session(path: &Path, budget: &mut ResourceBudget) -> OfflineWriteSession {
    OfflineWriteSession::open(path, CatalogScope::new(1), budget, &|| false).unwrap()
}
