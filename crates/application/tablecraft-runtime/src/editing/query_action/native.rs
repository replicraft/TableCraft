//! One bounded lease-free preparation path, with distinct native action plans.
use super::QueryActionTarget;
use crate::{
    Error, InspectedQuery, ParameterInput,
    controls::{SCOPE, action_query_budget, action_review_budget, check, comparison_budget},
    display::Presentation,
    document::Document,
    editing::EditError,
    query::parameters,
};
use std::{
    cell::RefCell,
    sync::{Arc, atomic::AtomicBool},
};
use tablecraft_access::{NativeSource, editing::sql};
use tablecraft_ace::{
    InspectError, NativeImage, RecoverableNativeDeletePlan, RecoverableNativeEditPlan,
};
use tablecraft_core::limits::Resource;
use tablecraft_query::{DeletePlan, UpdatePlan};
use tablecraft_sql::{ParseLimits, parse_query};

pub(in crate::editing) enum Kind {
    Update,
    Delete,
}

pub(in crate::editing) enum Plan {
    Update(UpdatePlan<RecoverableNativeEditPlan>),
    Delete(DeletePlan<RecoverableNativeDeletePlan>),
}

pub(in crate::editing) struct ReviewedAction {
    pub(in crate::editing) target: QueryActionTarget,
    pub(in crate::editing) plan: Plan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) fn review(
    document: &mut Document,
    target: &InspectedQuery,
    inputs: &[ParameterInput],
    output: &mut Presentation<'_>,
    cancelled: &dyn Fn() -> bool,
    kind: Kind,
) -> Result<ReviewedAction, Error> {
    let mut query_work = action_query_budget();
    query_work.charge(Resource::Bytes, target.sql().len() as u64)?;
    query_work.charge(Resource::Steps, target.sql().len() as u64)?;
    let parsed = parse_query(target.sql(), ParseLimits::default())?;
    let table = match kind {
        Kind::Update => sql::local_batch_update_table(parsed.query(), true)
            .map_err(sql::RecoverableError::Shape)?,
        Kind::Delete => sql::local_delete_table(parsed.query(), true)
            .map_err(sql::RecoverableDeleteError::Shape)?,
    };
    let bindings = parameters::bindings(parsed.query(), inputs, output)?;
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }

    let native_pool = RefCell::new(action_review_budget(document.edits.original.len())?);
    let image = {
        let mut native = native_pool.borrow_mut();
        NativeImage::capture(&mut document.source, SCOPE, &mut native, cancelled)?
    };

    let source =
        NativeSource::new_with_shared_budget(&mut document.source, SCOPE, &native_pool, cancelled)?;
    let comparison_pool = RefCell::new(comparison_budget());
    let (_source, resolver) = source.with_deferred_database_collation_resolver(&comparison_pool)?;

    let plan = {
        let mut native = native_pool
            .try_borrow_mut()
            .map_err(|_| Error::Native(InspectError::Unsupported))?;
        let options = sql::NativeMutationOptions {
            parameters: &bindings,
            collation: resolver.default_collation(),
            resolver: &resolver,
        };
        match kind {
            Kind::Update => Plan::Update(sql::plan_recoverable_batch_update_with_options(
                image,
                parsed.query(),
                &mut native,
                &mut query_work,
                options,
                cancelled,
            )?),
            Kind::Delete => Plan::Delete(sql::plan_recoverable_delete_with_options(
                image,
                parsed.query(),
                &mut native,
                &mut query_work,
                options,
                cancelled,
            )?),
        }
    };

    check(cancelled)?;
    let table = output.text(table)?;
    {
        let mut native = native_pool
            .try_borrow_mut()
            .map_err(|_| Error::Native(InspectError::Unsupported))?;
        native.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
        native.charge_allocation(1, size_of::<ReviewedAction>() as u64)?;
    }

    Ok(ReviewedAction {
        target: QueryActionTarget {
            path: document.edits.path.clone(),
            document_revision: document.revision,
            table,
        },
        plan,
        valid: Arc::clone(&document.edits.valid),
    })
}
