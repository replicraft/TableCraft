//! Explicit fresh engineering copies; normal native saves remain separate.
mod args;
mod error;
mod input;
mod publication;
mod run;
mod updates;
mod values;

pub(crate) use args::Command;
use error::Error;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

fn tick(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), Error> {
    if cancelled() {
        return Err(InspectError::Cancelled.into());
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)?;
    Ok(())
}

impl Command {
    pub(crate) fn run(self, password: Option<&str>) -> Result<String, String> {
        run::run(self, password).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests;
