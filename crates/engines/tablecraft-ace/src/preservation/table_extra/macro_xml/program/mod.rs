mod model;
mod read;
mod text;
mod tree;

pub use model::{
    DataMacroProgram, MacroProgramCoverage, MacroProgramDiagnostic, MacroProgramNode,
    MacroProgramNodeKind, MacroProgramProfile, MacroProgramText, MacroProgramTextKind, XmlByteSpan,
};

use super::super::TableExtraProperty;
use crate::InspectError;
use tablecraft_core::limits::ResourceBudget;

impl TableExtraProperty {
    /// Internal complete before-event structural inspection. This grants no
    /// execution or write permission; the complete-field compiler separately
    /// validates each declared event's actions, bindings and exact source.
    #[cfg(feature = "ace16-write-preview")]
    pub(crate) fn inspect_complete_before_events(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DataMacroProgram<'_>, InspectError> {
        read::inspect_complete_before_events(self, budget, cancelled)
    }

    /// Inspect an explicitly selected macro structural profile without evaluating
    /// conditions/arguments, executing actions or certifying absence/write safety.
    /// Original native UTF-16 bytes and decoded XML byte coordinates stay distinct.
    pub fn inspect_macro_program(
        &self,
        profile: MacroProgramProfile,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DataMacroProgram<'_>, InspectError> {
        read::inspect(self, profile, budget, cancelled)
    }
}

#[cfg(test)]
mod tests;
