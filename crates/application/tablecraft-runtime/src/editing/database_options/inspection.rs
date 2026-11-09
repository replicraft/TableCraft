//! Inert stored settings and positive source eligibility, never mutation authority.
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    document::Document,
    editing::platform,
};
use tablecraft_ace::{DatabaseNameAutoCorrect, inspect_database_properties};

/// Actual stored policy classification; no absent value selects a default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameAutoCorrectState {
    /// No qualifying native MSysDb role was found.
    Missing,
    /// The native database role has neither named tracking setting.
    Absent,
    /// Both settings are explicitly represented by qualified stored zero values.
    ExplicitOff,
    /// Partial, duplicate, differently typed or other stored settings.
    Other,
}

/// Facts from the exact current read session, without a save capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DatabaseOptionsInspection {
    pub document_revision: u64,
    pub name_auto_correct: NameAutoCorrectState,
    pub disable_available: bool,
}

impl Document {
    pub(crate) fn inspect_database_options(
        &mut self,
        document_revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        let _span = tracing::debug_span!("application.inspect_database_options").entered();
        if document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        check(cancelled)?;
        let mut work = budget();
        let stored = inspect_database_properties(&mut self.source, SCOPE, &mut work, cancelled)?;
        let name_auto_correct = match stored {
            None => NameAutoCorrectState::Missing,
            Some(properties) => match properties.name_auto_correct(&mut work, cancelled)? {
                DatabaseNameAutoCorrect::Absent => NameAutoCorrectState::Absent,
                DatabaseNameAutoCorrect::ExplicitOff => NameAutoCorrectState::ExplicitOff,
                DatabaseNameAutoCorrect::Other => NameAutoCorrectState::Other,
            },
        };
        let disable_available =
            platform::database_options::source_available(self, &mut work, cancelled)?;
        check(cancelled)?;
        Ok(Response::DatabaseOptionsInspected {
            inspection: DatabaseOptionsInspection {
                document_revision: self.revision,
                name_auto_correct,
                disable_available,
            },
        })
    }
}
