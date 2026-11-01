//! One qualified original state selects a closed operation before page staging.
use crate::{
    EditError, FittedColumnUpdates, NativeImage,
    header::check,
    table::mutation::{
        normal_fitted::{binding, owned_growth::selection::requires_growth, stage as records},
        normal_ordinary,
        record_placement::{future::Future, owned::Reservations, space::Space},
        record_roots,
        record_serialization::policy::RecordPolicy,
    },
};
use tablecraft_core::limits::ResourceBudget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OrdinaryVariablePlanKind {
    Fitted,
    Owned,
    AllocateEof,
}

impl NativeImage {
    pub(crate) fn ordinary_variable_plan_kind(
        &self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<OrdinaryVariablePlanKind, EditError> {
        let _span = tracing::debug_span!("native.ordinary.variable.select_plan").entered();
        check(budget, cancelled)?;
        let source = normal_ordinary::capture(self, table, budget, cancelled)?;
        // Complete old fitting preflight, including empty groups and stale
        // carriers, precedes native17/EOF qualification. Global15 stays fitted.
        if !requires_growth(self, &source, groups, budget, cancelled)? {
            return Ok(OrdinaryVariablePlanKind::Fitted);
        }
        let bound = binding::all(&source, groups, budget, cancelled)?;
        let captured =
            record_roots::capture(self, source.table.schema(), &bound.rows, budget, cancelled)?;
        let space = Space::capture_normal(
            self,
            source.table.schema(),
            &captured.owned,
            budget,
            cancelled,
        )?;
        let (records, _) = records::prepare_with(
            self,
            &source,
            groups,
            RecordPolicy::GrowingPreferred,
            budget,
            cancelled,
        )?;
        if super::stage::owned_fits(
            self,
            &space,
            &captured.owned,
            &captured.selected,
            &records,
            budget,
            cancelled,
        )? {
            return Ok(OrdinaryVariablePlanKind::Owned);
        }
        let future = Future::capture(self, source.table.schema(), budget, cancelled)?;
        let mut reservations =
            Reservations::capture(self, &space, &captured.owned, budget, cancelled)?;
        reservations.include_eof(self, &future, budget, cancelled)?;
        let mut used = false;
        for root in &captured.selected {
            for record in &records {
                check(budget, cancelled)?;
                if root.final_record.offset == record.offset {
                    let reservation =
                        reservations.reserve(self, root, record.after.len(), budget, cancelled)?;
                    used |= reservation
                        .destination()
                        .is_some_and(|target| target.page == future.number());
                }
            }
        }
        if !used {
            return Err(EditError::Preservation);
        }
        check(budget, cancelled)?;
        Ok(OrdinaryVariablePlanKind::AllocateEof)
    }
}
