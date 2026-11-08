//! One bounded lease-free preparation path, with distinct native action plans.
use crate::editing::query_action::QueryActionTarget;
use crate::{
    Error, InspectedQuery, ParameterInput,
    controls::{
        SCOPE, action_query_budget, action_query_limits, action_review_budget, check,
        comparison_budget,
    },
    display::Presentation,
    document::Document,
    editing::EditError,
    query::{context::Context, parameters},
};
use std::{
    cell::RefCell,
    sync::{Arc, atomic::AtomicBool},
};
use tablecraft_access::{
    NativeSource,
    editing::{
        journal::{NativeJournalInsertPlan, NativeJournalInserter},
        sql,
    },
};
use tablecraft_ace::{
    DatabaseQueryMode, InspectError, NativeImage, RecoverableNativeDeletePlan,
    RecoverableNativeEditPlan,
};
use tablecraft_core::limits::Resource;
use tablecraft_expressions::{functions::dates::clock::Clock, predicates::LikeMode};
use tablecraft_query::{DeletePlan, ExecutionOptions, UpdatePlan};
use tablecraft_sql::{
    ParseLimits, parse_query,
    query::{InsertSource, Statement},
};

#[derive(Clone, Copy)]
pub(in crate::editing) enum Kind<'a> {
    Update,
    Delete,
    Insert(&'a dyn Clock),
}

pub(in crate::editing) enum Plan {
    Update(UpdatePlan<RecoverableNativeEditPlan>),
    Delete(DeletePlan<RecoverableNativeDeletePlan>),
    Insert(NativeJournalInsertPlan),
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
    kind: Kind<'_>,
) -> Result<ReviewedAction, Error> {
    let mut query_work = action_query_budget();
    query_work.charge(Resource::Bytes, target.sql().len() as u64)?;
    query_work.charge(Resource::Steps, target.sql().len() as u64)?;
    let parsed = parse_query(target.sql(), ParseLimits::default())?;
    let select_insert = matches!(
        &parsed.query().statement,
        Statement::Insert(insert) if matches!(insert.source, InsertSource::Query(_))
    );
    let table = match kind {
        Kind::Update => sql::local_batch_update_table(parsed.query(), true)
            .map_err(sql::RecoverableError::Shape)?,
        Kind::Delete => sql::local_delete_table(parsed.query(), true)
            .map_err(sql::RecoverableDeleteError::Shape)?,
        Kind::Insert(_) if select_insert => {
            sql::local_insert_select_table(parsed.query(), &mut query_work, cancelled)
                .map_err(sql::RecoverableInsertError::Shape)?
        }
        Kind::Insert(_) => {
            sql::local_insert_table(parsed.query(), true, &mut query_work, cancelled)
                .map_err(sql::RecoverableInsertError::Shape)?
        }
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
    let (mut source, resolver) =
        source.with_deferred_database_collation_resolver(&comparison_pool)?;

    let plan = if let Kind::Insert(clock) = kind
        && select_insert
    {
        let inserter = {
            let mut native = native_pool
                .try_borrow_mut()
                .map_err(|_| Error::Native(InspectError::Unsupported))?;
            NativeJournalInserter::new(image, table, &mut native, cancelled)
                .map_err(sql::RecoverableInsertError::Editor)?
        };
        let context = Context::new(clock);
        let like_mode = match source.database_query_mode()? {
            DatabaseQueryMode::Ansi89 => Some(LikeMode::Ansi89),
            DatabaseQueryMode::Ansi92 => Some(LikeMode::Ansi92),
            DatabaseQueryMode::Absent | DatabaseQueryMode::Unknown => None,
        };

        // Source callbacks borrow the native pool during retrieval. Retain no
        // mutable budget borrow until the complete SELECT has been staged.
        let staged = sql::prepare_native_insert_select(
            inserter,
            parsed.query(),
            &source,
            &mut query_work,
            &bindings,
            ExecutionOptions {
                cancelled: Some(cancelled),
                limits: action_query_limits(),
                collation: Some(resolver.default_collation()),
                collation_resolver: Some(&resolver),
                like_mode,
                ..context.options()
            },
            Some(clock),
        )?;
        let mut native = native_pool
            .try_borrow_mut()
            .map_err(|_| Error::Native(InspectError::Unsupported))?;
        Plan::Insert(staged.finish(&mut native)?)
    } else {
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
            Kind::Insert(clock) => Plan::Insert(sql::plan_recoverable_insert_with_options(
                image,
                parsed.query(),
                &mut native,
                &mut query_work,
                options,
                Some(clock),
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
