//! Injected discovery proves scheduling/cache/failure without OS font assumptions.
use super::{Catalog, FontInventory, spawn};
use egui::Context;
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::sync_channel,
    },
    time::{Duration, Instant},
};
use tablecraft_ui::FontFamilies;

#[test]
fn discovery_is_background_once_and_both_hosts_observe_the_same_cached_catalog() {
    static CATALOG: Catalog = Catalog::new();
    let (entered, entry) = sync_channel(1);
    let (release, released) = sync_channel(1);
    let caller = std::thread::current().id();
    let count = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&count);
    let context = Context::default();
    CATALOG.start_with(
        &context,
        move || {
            calls.fetch_add(1, Ordering::AcqRel);
            entered.send(std::thread::current().id()).unwrap();
            released.recv().unwrap();
            Some(FontFamilies::from_system(["Injected family"]).unwrap())
        },
        spawn,
    );
    assert_ne!(entry.recv_timeout(Duration::from_secs(2)).unwrap(), caller);
    let mut desktop = FontInventory::default();
    let mut synthetic = FontInventory::default();
    assert!(desktop.poll_catalog(&CATALOG, &context).is_none());
    CATALOG.start_with(
        &context,
        || panic!("cached task must not rerun"),
        |_| {
            panic!("second host must not spawn another discovery");
        },
    );
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let first = loop {
        if let Some(catalog) = desktop.poll_catalog(&CATALOG, &context) {
            break catalog;
        }
        assert!(Instant::now() < deadline, "injected discovery must publish");
        std::thread::yield_now();
    };
    let second = synthetic.poll_catalog(&CATALOG, &context).unwrap();
    assert_eq!(first.families().as_ptr(), second.families().as_ptr());
    assert!(
        first
            .families()
            .iter()
            .any(|name| name == "Injected family")
    );
    assert_eq!(count.load(Ordering::Acquire), 1);
    assert!(desktop.poll_catalog(&CATALOG, &context).is_none());
}

#[test]
fn spawn_failure_keeps_bundled_fonts_and_never_executes_or_retries_discovery() {
    static CATALOG: Catalog = Catalog::new();
    let context = Context::default();
    CATALOG.start_with(
        &context,
        || panic!("failed spawn must not discover"),
        |_| Err(io::ErrorKind::WouldBlock.into()),
    );
    let mut inventory = FontInventory::default();
    assert!(inventory.poll_catalog(&CATALOG, &context).is_none());
    assert!(inventory.finished);
    CATALOG.start_with(
        &context,
        || panic!("failed process task must not retry"),
        |_| panic!("no second spawn"),
    );
    let bundled = FontFamilies::default();
    assert!(bundled.families().iter().any(|name| name == "Selawik"));
}

#[test]
fn provider_panic_is_terminal_fallback_and_does_not_block_host_polling() {
    static CATALOG: Catalog = Catalog::new();
    let context = Context::default();
    CATALOG.start_with(
        &context,
        || panic!("injected font discovery failure"),
        spawn,
    );
    let mut inventory = FontInventory::default();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !inventory.finished {
        assert!(inventory.poll_catalog(&CATALOG, &context).is_none());
        assert!(Instant::now() < deadline, "failed discovery must finish");
        std::thread::yield_now();
    }
    assert!(CATALOG.families.get().is_none());
}
