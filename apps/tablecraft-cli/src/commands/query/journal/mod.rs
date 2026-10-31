//! Explicit offline normal Jet4 editing with retained journal pairing.
mod args;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod confirmation;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod error;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
#[path = "unsupported.rs"]
mod error;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod preflight;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod report;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod runner;
#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests;

pub(crate) use args::{FinalizeArguments, RecoveryArguments, UpdateArguments};
pub(in crate::commands) use error::Error;
use std::io::Write;

pub(in crate::commands) fn update(
    args: UpdateArguments,
    password: Option<&str>,
    writer: &mut dyn Write,
) -> Result<(), Error> {
    reject_password(password)?;

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        runner::update(args, writer)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (args, writer);
        Err(Error::UnsupportedPlatform)
    }
}

pub(in crate::commands) fn recover(
    args: RecoveryArguments,
    password: Option<&str>,
    writer: &mut dyn Write,
) -> Result<(), Error> {
    reject_password(password)?;

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        runner::recover(args, writer)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (args, writer);
        Err(Error::UnsupportedPlatform)
    }
}

fn reject_password(password: Option<&str>) -> Result<(), Error> {
    if password.is_some() {
        return Err(Error::Credentials);
    }

    Ok(())
}

pub(in crate::commands) fn finalize(
    args: FinalizeArguments,
    password: Option<&str>,
    writer: &mut dyn Write,
) -> Result<(), Error> {
    reject_password(password)?;

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        confirmation::run(args, writer)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (args, writer);
        Err(Error::UnsupportedPlatform)
    }
}
