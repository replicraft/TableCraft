//! Explicit side-effect-free before-event preparation over immutable native intents.
mod authority;
mod buffer;
mod compile;
mod error;
mod evaluate;
mod expression;
mod model;
mod selection;
mod source;

pub use error::{BeforeEventGuardError, BeforeGuardSelectionError, BeforeGuardUnsupported};
pub use model::{
    BeforeEvent, BeforeEventGuardPreparation, BeforeEventGuardSelector, BeforeEventIntents,
    BeforeGuardProgramRevision, BeforeGuardRejection, BeforeGuardTrace, BeforeGuardTraceKind,
};

use crate::{NativeImage, header::check};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Hard ceiling for one explicit source-property selection list.
    pub const MAX_BEFORE_GUARD_PROPERTIES: usize = 1024;

    /// Prepare the selected documented before-event guard subset over original
    /// source and exact existing intents. This never creates a writable candidate,
    /// executes external effects, proves macro absence or permits a normal save.
    /// The nonempty exact source-property selectors define caller engineering
    /// order, not native dispatch. Unselected XML is never run; selecting this
    /// subset does not detect native macro storage or required-macro coverage.
    ///
    /// The initial source profile is ordinary Int32 fields with all insert values
    /// supplied. If is Boolean-only. Updated explicitly means actual native-value
    /// change; equal-assignment/default/generated/cascade native timing is not
    /// certified. Unsupported syntax/context or failures return no completed report.
    pub fn prepare_ace16_before_event_guards(
        &self,
        table_name: &str,
        selectors: &[BeforeEventGuardSelector<'_>],
        original: BeforeEventIntents<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<BeforeEventGuardPreparation<'_>, BeforeEventGuardError> {
        let _span = tracing::debug_span!("native.before_event.prepare").entered();
        check(budget, cancelled)?;
        self.require_standard_capture()?;
        selection::validate(selectors, budget, cancelled)?;
        let event = original.event();
        let table = source::table(self, table_name, budget, cancelled)?;
        let frames = source::frames(self, &table, original, budget, cancelled)?;
        let compiled = compile::derive(self, &table, selectors, event, budget, cancelled)?;
        let (trace, rejection) = evaluate::run(&compiled, &frames, event, budget, cancelled)?;

        authority::prove(
            self,
            table_name,
            selectors,
            original,
            authority::Observed {
                compiled: &compiled,
                trace: &trace,
                rejection: rejection.as_ref(),
            },
            budget,
            cancelled,
        )?;
        check(budget, cancelled)?;
        Ok(BeforeEventGuardPreparation {
            source: self.as_bytes(),
            event,
            trace,
            rejection,
            object: table.schema().object_id(),
            programs: compiled.programs,
        })
    }
}
