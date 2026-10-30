mod model;
mod read;
mod tree;

pub use model::{
    DataMacroProgram, MacroProgramCoverage, MacroProgramDiagnostic, MacroProgramNode,
    MacroProgramNodeKind, MacroProgramProfile, XmlByteSpan,
};

use super::super::TableExtraProperty;
use crate::InspectError;
use tablecraft_core::limits::ResourceBudget;

impl TableExtraProperty {
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
