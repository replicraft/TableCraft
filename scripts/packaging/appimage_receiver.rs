//! Disposable headless AppImage qualification receiver; never a release payload.

use std::{env, fs::OpenOptions, io::Write, thread, time::Duration};

fn main() {
    let version = env!("TABLECRAFT_TEST_RECEIVER_VERSION");
    if env::args().nth(1).as_deref() == Some("--version") {
        println!("tablecraft-headless-update-receiver {version}");
        return;
    }

    let receipt = env::var_os("TABLECRAFT_TEST_APPIMAGE_RECEIPT")
        .expect("qualification requires an explicit receipt path");
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(receipt)
        .expect("qualification receipt must be fresh");
    writeln!(output, "receiver=tablecraft-headless-update-receiver").unwrap();
    writeln!(output, "version={version}").unwrap();
    writeln!(output, "pid={}", std::process::id()).unwrap();
    for name in ["APPIMAGE", "APPDIR", "ARGV0", "OWD"] {
        writeln!(output, "{name}={}", env::var(name).unwrap_or_default()).unwrap();
    }
    output.sync_all().unwrap();
    if env::var_os("TABLECRAFT_TEST_APPIMAGE_FAIL").is_some() {
        std::process::exit(23);
    }

    // The native helper's health observation occurs after one second. Remaining
    // alive beyond it exercises cleanup; no window, renderer or event loop exists.
    thread::sleep(Duration::from_secs(5));
}
