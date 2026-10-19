use super::super::{Ace16WritePreview, EditError, WriteBlocker, preview::qualify};
use super::{FixedScalarUpdate, encoding, update::EncodedUpdate};
use crate::{
    InspectError, NativeImage, header::check, inspect_table_schema, limits::MAX_LIVE_ROWS,
};
use tablecraft_core::{limits::ResourceBudget, values::Guid};
use tablecraft_schema::ScalarType;

impl NativeImage {
    /// Preview exact fixed, non-NULL scalar replacements on ACE 16.
    ///
    /// Supports Byte, Int16/32/64, Currency, Single, Double, legacy DateTime and
    /// GUID with exact carriers and original bits. Only Int32 index mutation is
    /// currently qualified; other indexed targets fail. Boolean masks, NULL
    /// transitions, variable fields and Extended DateTime remain separate work.
    /// This optional engineering API retains all preview qualifications and
    /// blockers and never produces a journalable plan or writes a source.
    pub fn preview_ace16_fixed_updates(
        self,
        table: &str,
        column: &str,
        updates: &[FixedScalarUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span = tracing::debug_span!("native.edit.fixed_preview", assignments = updates.len())
            .entered();
        check(budget, cancelled)?;
        qualify(&self)?;
        if updates.len() > MAX_LIVE_ROWS {
            return Err(InspectError::StructureLimit.into());
        }
        let schema = inspect_table_schema(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        let selected = schema
            .columns()
            .iter()
            .find(|field| field.name().eq_ignore_ascii_case(column))
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        if !selected.fixed() {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        let kind = selected
            .scalar_type()
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        match kind {
            ScalarType::Byte => generate::<{ size_of::<u8>() }>(
                self, table, column, updates, kind, budget, cancelled,
            ),
            ScalarType::Int16 => generate::<{ size_of::<i16>() }>(
                self, table, column, updates, kind, budget, cancelled,
            ),
            ScalarType::Int32 | ScalarType::Single => generate::<{ size_of::<i32>() }>(
                self, table, column, updates, kind, budget, cancelled,
            ),
            ScalarType::Int64
            | ScalarType::Currency
            | ScalarType::Double
            | ScalarType::DateTime => generate::<{ size_of::<i64>() }>(
                self, table, column, updates, kind, budget, cancelled,
            ),
            ScalarType::Guid => generate::<{ Guid::BYTE_LEN }>(
                self, table, column, updates, kind, budget, cancelled,
            ),
            _ => Err(EditError::Unsupported(WriteBlocker::Column)),
        }
    }
}

fn generate<const WIDTH: usize>(
    image: NativeImage,
    table: &str,
    column: &str,
    updates: &[FixedScalarUpdate],
    kind: ScalarType,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    budget
        .charge_allocation(
            updates.len() as u64,
            size_of::<EncodedUpdate<WIDTH>>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut encoded: Vec<EncodedUpdate<WIDTH>> = Vec::new();
    encoded
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    for update in updates {
        check(budget, cancelled)?;
        encoded.push(EncodedUpdate {
            row: update.row(),
            expected: encoding::encode(kind, update.expected())?,
            value: encoding::encode(kind, update.value())?,
        });
    }
    image
        .plan_fixed_updates_inner(table, column, &encoded, kind, budget, cancelled)
        .map(Ace16WritePreview::from_plan)
}
