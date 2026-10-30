use super::super::{Ace16WritePreview, EditError, WriteBlocker, preview::qualify};
use super::{FixedColumnUpdates, FixedScalarUpdate, masked, preserved};
use crate::{
    InspectError, NativeImage, format::data_type::NUMERIC_BYTES, header::check,
    inspect_table_schema, limits::MAX_LIVE_ROWS,
};
use tablecraft_core::{limits::ResourceBudget, values::Guid};
use tablecraft_schema::ScalarType;

impl NativeImage {
    /// Preview exact scalar payload replacements and row-mask transitions on ACE 16.
    ///
    /// Supports Boolean, Byte, Int16/32/64, Currency, Single, Double, legacy DateTime,
    /// GUID and qualified Decimal slots. Decimal uses an existing 17-byte variable
    /// slot, with exact field precision/scale and no record/offset resizing. NULL
    /// is allowed for eligible non-Boolean fields after required/validation and
    /// relationship checks. Qualified single/composite index maintenance repacks
    /// existing pages; allocation/splits remain unsupported. Text/binary and
    /// Extended DateTime mutations remain separate work.
    /// Eligible foreign indexes require complete reciprocal/catalog and before/
    /// after membership checks. No cascade action is executed by this method;
    /// the separate cascading-batch API expands eligible original associations.
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
        if preserved::projected(&schema, budget, cancelled)? {
            return preserved::preview(
                self,
                &[FixedColumnUpdates::new(table, column, updates)],
                budget,
                cancelled,
            );
        }
        let selected = schema
            .columns()
            .iter()
            .find(|field| field.name().eq_ignore_ascii_case(column))
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        if !selected.fixed() && selected.scalar_type() != Some(ScalarType::Decimal) {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        let kind = selected
            .scalar_type()
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        match kind {
            ScalarType::Boolean => {
                masked::generate::<0>(self, table, column, updates, kind, budget, cancelled)
            }
            ScalarType::Byte => masked::generate::<{ size_of::<u8>() }>(
                self, table, column, updates, kind, budget, cancelled,
            ),
            ScalarType::Int16 => masked::generate::<{ size_of::<i16>() }>(
                self, table, column, updates, kind, budget, cancelled,
            ),
            ScalarType::Int32 | ScalarType::Single => masked::generate::<{ size_of::<i32>() }>(
                self, table, column, updates, kind, budget, cancelled,
            ),
            ScalarType::Int64
            | ScalarType::Currency
            | ScalarType::Double
            | ScalarType::DateTime => masked::generate::<{ size_of::<i64>() }>(
                self, table, column, updates, kind, budget, cancelled,
            ),
            ScalarType::Decimal => masked::generate::<NUMERIC_BYTES>(
                self, table, column, updates, kind, budget, cancelled,
            ),
            ScalarType::Guid => masked::generate::<{ Guid::BYTE_LEN }>(
                self, table, column, updates, kind, budget, cancelled,
            ),
            _ => Err(EditError::Unsupported(WriteBlocker::Column)),
        }
    }
}
