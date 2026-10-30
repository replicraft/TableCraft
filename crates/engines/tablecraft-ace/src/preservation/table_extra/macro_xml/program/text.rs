use super::{DataMacroProgram, MacroProgramNodeKind, MacroProgramText, MacroProgramTextKind};
use crate::{InspectError, header::check};
use tablecraft_core::limits::ResourceBudget;

impl DataMacroProgram<'_> {
    /// Decode one recognized Condition/argument without evaluating expressions.
    /// Missing Description returns None; present-empty returns an empty value.
    /// Other node/slot combinations refuse rather than stripping opaque markup.
    pub fn text_content(
        &self,
        node: usize,
        kind: MacroProgramTextKind,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<MacroProgramText>, InspectError> {
        let _span = tracing::debug_span!("native.table.extra.macro_program.text").entered();
        check(budget, cancelled)?;
        let node = self.nodes().get(node).ok_or(InspectError::Malformed)?;
        let source = match (node.kind(), kind) {
            (MacroProgramNodeKind::Condition { content }, MacroProgramTextKind::Condition) => {
                Some(content)
            }
            (MacroProgramNodeKind::RaiseError { number, .. }, MacroProgramTextKind::Number) => {
                Some(number)
            }
            (
                MacroProgramNodeKind::RaiseError { description, .. },
                MacroProgramTextKind::Description,
            ) => description,
            _ => return Err(InspectError::Unsupported),
        };

        let Some(source) = source else {
            check(budget, cancelled)?;
            return Ok(None);
        };
        let raw = self.source_span(source)?;
        let value = super::super::read::text::content(raw, budget, cancelled)?;

        check(budget, cancelled)?;
        Ok(Some(MacroProgramText { source, value }))
    }
}
