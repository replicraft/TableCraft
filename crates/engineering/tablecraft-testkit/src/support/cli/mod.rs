//! CLI process and readback test workflows with explicit executables.
pub mod owned_growth;

use std::process::{Command, Output};

pub struct Runner {
    binary: &'static str,
}

impl Runner {
    pub const fn new(binary: &'static str) -> Self {
        Self { binary }
    }

    pub fn run(&self, args: &[&str]) -> Output {
        Command::new(self.binary)
            .args(args)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .expect("CLI should launch without a display")
    }
}
