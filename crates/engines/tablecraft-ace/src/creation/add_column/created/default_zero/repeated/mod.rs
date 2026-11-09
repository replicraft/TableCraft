//! Separate source-qualified repeated Number addition; frozen first-ADD stays strict.
mod build;
mod source;
#[cfg(test)]
mod tests;
mod verify;

use super::CreatedDefaultZeroAddColumnError as Error;
use crate::{
    CreateColumn, NativeImage,
    creation::add_column::{AddColumnCandidate, owned},
    header::check,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

/// Candidate for a separately proved bounded property-LVAL history.
/// It has no first-ADD intent, journal or normal-save conversion.
pub struct RepeatedDefaultZeroAddColumnCandidate {
    image: AddColumnCandidate,
}

impl NativeImage {
    /// Observe this exact external-property Number source without write authority.
    pub fn check_repeated_default_zero_add_column_source(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Error> {
        source::capture(self, table, budget, cancelled)?;
        Ok(())
    }

    /// Append a nullable LONG with explicit Required(false) and DefaultValue zero.
    ///
    /// The complete source must equal a canonical generated-primary seed followed
    /// by the original first Number addition and exact property-row appends.
    /// The owned ledger is page 10 or pages 10 and 11. One immediate-EOF
    /// rollover may add page 11; subsequent bodies must fit there. Earlier rows
    /// remain unchanged, and no second allocation or page reuse is admitted.
    /// This separate candidate never widens TCAC0001/0002/0003 admission.
    pub fn prepare_repeated_default_zero_int32_column_candidate(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RepeatedDefaultZeroAddColumnCandidate, Error> {
        let _span = tracing::debug_span!("native.repeated_number.prepare_candidate").entered();
        check(budget, cancelled)?;
        crate::creation::request::name(column, budget, cancelled)?;
        let source = source::capture(&self, table, budget, cancelled)?;
        let mut columns = source.definition.borrowed_columns(budget, cancelled)?;
        budget
            .charge_allocation(1, size_of::<CreateColumn<'_>>() as u64)
            .map_err(crate::InspectError::from)?;
        columns
            .try_reserve_exact(1)
            .map_err(|_| crate::InspectError::Allocation)?;
        let zero = Value::Int32(0);
        columns.push(
            CreateColumn::new(column, ScalarType::Int32)
                .with_default_literal(&zero)?
                .with_required(false),
        );
        let after = build::append(
            &self,
            &source.checked,
            &columns,
            source.default_first,
            budget,
            cancelled,
        )?;
        let table = owned(source.checked.schema().name(), budget)?;
        let column = owned(column, budget)?;
        let count = columns.len();
        drop(columns);
        check(budget, cancelled)?;
        Ok(RepeatedDefaultZeroAddColumnCandidate {
            image: AddColumnCandidate {
                before: self,
                after,
                table,
                column,
                columns: count,
            },
        })
    }
}

impl RepeatedDefaultZeroAddColumnCandidate {
    /// Complete source bytes whose external-property history was reconstructed.
    pub fn before_bytes(&self) -> &[u8] {
        self.image.before_bytes()
    }

    /// Complete proposed image, with at most one qualified EOF page addition.
    pub fn after_bytes(&self) -> &[u8] {
        self.image.after_bytes()
    }

    /// Exact native table spelling.
    pub fn table(&self) -> &str {
        self.image.table()
    }

    /// Exact proposed field spelling.
    pub fn column(&self) -> &str {
        self.image.column()
    }

    /// Field count after the addition.
    pub fn column_count(&self) -> usize {
        self.image.column_count()
    }
}
