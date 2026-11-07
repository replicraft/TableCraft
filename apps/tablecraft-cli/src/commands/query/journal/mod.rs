//! Explicit offline qualified native editing with retained journal pairing.
mod args;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod confirmation;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod delete;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod error;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
#[path = "unsupported.rs"]
mod error;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod insert;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod preflight;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod report;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod runner;
#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;

pub(crate) use args::{
    DeleteArguments, FinalizeArguments, InsertArguments, RecoveryArguments, UpdateArguments,
};
pub(in crate::commands) use error::Error;
use std::io::Write;

pub(in crate::commands) fn update(
    args: UpdateArguments,
    password: Option<&str>,
    writer: &mut dyn Write,
) -> Result<(), Error> {
    reject_password(password)?;

    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    {
        runner::update(args, writer)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
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

    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    {
        runner::recover(args, writer)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
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

    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    {
        confirmation::run(args, writer)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        let _ = (args, writer);
        Err(Error::UnsupportedPlatform)
    }
}

pub(in crate::commands) fn delete(
    args: DeleteArguments,
    password: Option<&str>,
    writer: &mut dyn Write,
) -> Result<(), Error> {
    reject_password(password)?;

    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    {
        delete::run(args, writer)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        let _ = (args, writer);
        Err(Error::UnsupportedPlatform)
    }
}

pub(in crate::commands) fn insert(
    args: InsertArguments,
    password: Option<&str>,
    writer: &mut dyn Write,
) -> Result<(), Error> {
    reject_password(password)?;

    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    {
        insert::run(args, writer)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        let _ = (args, writer);
        Err(Error::UnsupportedPlatform)
    }
}
