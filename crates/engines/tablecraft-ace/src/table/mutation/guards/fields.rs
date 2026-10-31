//! Explicit BeforeChange row transformations, separate from guard-only behavior.
mod authority;
mod evaluate;
mod model;
pub(in crate::table::mutation) mod physical;

pub use model::{BeforeChangeAssignment, BeforeChangeFieldPreparation, BeforeChangeFinalRow};
pub use physical::{Ace16BeforeChangePreview, BeforeChangePreviewError};

use super::{
    BeforeEvent, BeforeEventGuardError, BeforeEventGuardSelector, BeforeEventIntents,
    BeforeGuardUnsupported, compile, source,
};
use crate::{NativeImage, header::check};
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};

impl NativeImage {
    /// Prepare a complete explicitly selected BeforeChange Int32/NULL program
    /// profile. Current changes sequentially; Old remains the original source.
    /// Every parsed Extra property must be selected and compiled. Updated and
    /// general EditRecord/CreateRecord contexts remain unsupported.
    ///
    /// A rejection discards all final rows. This method creates no candidate,
    /// executes no logging/external effects and grants no normal-save permission.
    pub fn prepare_ace16_before_change_fields(
        &self,
        table_name: &str,
        selectors: &[BeforeEventGuardSelector<'_>],
        original: BeforeEventIntents<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<BeforeChangeFieldPreparation<'_>, BeforeEventGuardError> {
        let _span = tracing::debug_span!("native.before_change.fields.prepare").entered();
        check(budget, cancelled)?;
        self.require_standard_capture()?;
        let prepared = prepare(self, table_name, selectors, original, budget, cancelled)?;
        let execution = prepared.execution;

        check(budget, cancelled)?;
        Ok(BeforeChangeFieldPreparation {
            source: self.as_bytes(),
            object: prepared.object,
            trace: execution.trace,
            assignments: execution.assignments,
            rejection: execution.rejection,
            rows: execution.rows,
            programs: prepared.compiled.programs,
        })
    }
}

#[cfg(test)]
mod physical_sources;

struct Prepared {
    object: ObjectId,
    compiled: compile::Compiled,
    execution: evaluate::Execution,
}

fn prepare(
    image: &NativeImage,
    table_name: &str,
    selectors: &[BeforeEventGuardSelector<'_>],
    original: BeforeEventIntents<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared, BeforeEventGuardError> {
    let event = original.event();
    if event == BeforeEvent::Delete {
        return Err(BeforeEventGuardError::Unsupported(
            BeforeGuardUnsupported::Context,
        ));
    }
    let table = source::table(image, table_name, budget, cancelled)?;
    let frames = source::frames(image, &table, original, budget, cancelled)?;
    let compiled = compile::derive_fields(image, &table, selectors, event, budget, cancelled)?;
    let execution = evaluate::run(&compiled, &frames, event, budget, cancelled)?;
    authority::prove(
        image,
        table_name,
        selectors,
        original,
        authority::Observed {
            compiled: &compiled,
            execution: &execution,
        },
        budget,
        cancelled,
    )?;

    Ok(Prepared {
        object: table.schema().object_id(),
        compiled,
        execution,
    })
}
