//! Explicit headless release check; never downloads, installs or opens a window.

use semver::Version;
use std::error::Error;
use tablecraft_update::{Updater, platform::Platform};

fn main() -> Result<(), Box<dyn Error>> {
    let platform = Platform::detect().ok_or("Unsupported native update target")?;
    let updater = Updater::new(Version::parse(env!("CARGO_PKG_VERSION"))?, platform)?;
    match updater.check(&|| false)? {
        Some(available) => println!(
            "A newer signed release is available: {}",
            available.version()
        ),
        None => println!("No newer release is available"),
    }
    Ok(())
}
