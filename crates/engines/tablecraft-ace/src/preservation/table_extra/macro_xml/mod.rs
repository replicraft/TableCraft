mod model;
mod names;
mod program;
mod read;

pub use model::{DataMacroEnvelope, DataMacroXml, DataMacroXmlNamespace};
pub use program::{
    DataMacroProgram, MacroProgramCoverage, MacroProgramDiagnostic, MacroProgramNode,
    MacroProgramNodeKind, MacroProgramProfile, MacroProgramText, MacroProgramTextKind, XmlByteSpan,
};

use super::TableExtraProperty;
use crate::InspectError;
use tablecraft_core::limits::ResourceBudget;

impl TableExtraProperty {
    /// Inspect explicit data-macro XML envelopes in this property's validated
    /// text. Body elements remain uninterpreted. Unknown profiles and malformed
    /// documents refuse; a successful result never certifies macro absence or
    /// authorizes writes. No action, expression or external resource executes.
    pub fn inspect_macro_xml(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DataMacroXml, InspectError> {
        let text = self.text().ok_or(InspectError::Unsupported)?;
        read::document(text, budget, cancelled)
    }
}

#[cfg(test)]
mod tests;
