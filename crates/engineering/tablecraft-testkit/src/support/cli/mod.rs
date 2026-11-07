//! CLI process and readback test workflows with explicit executables.
pub mod owned_growth;

use std::{
    ffi::OsStr,
    process::{Command, Output},
};

pub struct Runner {
    binary: &'static str,
}

impl Runner {
    pub const fn new(binary: &'static str) -> Self {
        Self { binary }
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.run_os(args)
    }

    /// Preserve platform-native paths without converting them into UTF-8 text.
    pub fn run_os<I, S>(&self, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Command::new(self.binary)
            .args(args)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .expect("CLI should launch without a display")
    }
}
