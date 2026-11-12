//! One bounded lease-free preparation path, with distinct native action plans.
use crate::editing::query_action::QueryActionTarget;
use crate::{
    Error, InspectedQuery, ParameterInput, QueryExpressionFormats,
    controls::{
        SCOPE, action_query_budget, action_query_limits, action_review_budget, check,
        comparison_budget,
    },
    display::Presentation,
    document::Document,
    editing::EditError,
    query::{
        context::{Context, Environment},
        parameters,
    },
};
use std::{
    cell::RefCell,
    sync::{Arc, atomic::AtomicBool},
};
use tablecraft_access::{
    NativeSource,
    editing::{
        journal::{NativeJournalEditor, NativeJournalInsertPlan, NativeJournalInserter},
        sql,
    },
};
use tablecraft_ace::{InspectError, RecoverableNativeDeletePlan, RecoverableNativeEditPlan};
use tablecraft_core::limits::Resource;
use tablecraft_query::{DeletePlan, ExecutionOptions, UpdatePlan};
use tablecraft_sql::{
    ParseLimits, parse_query,
    query::{InsertSource, Statement},
};

#[derive(Clone, Copy)]
pub(in crate::editing) enum Kind<'a> {
    Update(QueryExpressionFormats),
    Delete(QueryExpressionFormats),
    Insert(Environment<'a>),
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
    let formats = match kind {
        Kind::Update(formats) | Kind::Delete(formats) => formats,
        Kind::Insert(environment) => environment.formats,
    };
    let mut query_work = action_query_budget();
    query_work.charge(Resource::Bytes, target.sql().len() as u64)?;
    query_work.charge(Resource::Steps, target.sql().len() as u64)?;
    let parsed = parse_query(target.sql(), ParseLimits::default())?;
    let select_insert = matches!(
        &parsed.query().statement,
        Statement::Insert(insert) if matches!(insert.source, InsertSource::Query(_))
    );
    let joined_update = if matches!(kind, Kind::Update(_))
        && matches!(&parsed.query().statement, Statement::Update(update)
            if !matches!(update.sources.as_slice(), [tablecraft_sql::select::TableSource::Table(_)]))
    {
        Some(
            sql::local_joined_update_target(parsed.query(), true, &mut query_work, cancelled)
                .map_err(sql::RecoverableError::Shape)?,
        )
    } else {
        None
    };
    let read_source_update = matches!(kind, Kind::Update(_))
        && joined_update.is_none()
        && sql::update_uses_read_source(parsed.query(), &mut query_work, cancelled)
            .map_err(sql::RecoverableError::Shape)?;
    let joined_delete = if matches!(kind, Kind::Delete(_))
        && matches!(&parsed.query().statement, Statement::Delete(delete)
            if !matches!(delete.sources.as_slice(), [tablecraft_sql::select::TableSource::Table(_)]))
    {
        Some(
            sql::local_joined_delete_target(parsed.query(), true, &mut query_work, cancelled)
                .map_err(sql::RecoverableDeleteError::Shape)?,
        )
    } else {
        None
    };
    let read_source_delete = matches!(kind, Kind::Delete(_))
        && joined_delete.is_none()
        && sql::delete_uses_read_source(parsed.query(), &mut query_work, cancelled)
            .map_err(sql::RecoverableDeleteError::Shape)?;
    let table = match kind {
        Kind::Update(_) => match joined_update {
            Some(target) => target.table,
            None => sql::local_batch_update_table(parsed.query(), true)
                .map_err(sql::RecoverableError::Shape)?,
        },
        Kind::Delete(_) => match joined_delete {
            Some(target) => target.table,
            None => sql::local_delete_table(parsed.query(), true)
                .map_err(sql::RecoverableDeleteError::Shape)?,
        },
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
        // NO_CLONE: NativeImage owns the checked source while Document keeps using it; this
        // shares immutable bytes instead of copying the database image.
        let source = document.edits.image_source.clone();
        match kind {
            Kind::Insert(_) => NativeJournalInserter::capture_shared_image(
                source,
                SCOPE,
                table,
                &mut native,
                cancelled,
            )?,
            Kind::Update(_) | Kind::Delete(_) => NativeJournalEditor::capture_shared_image(
                source,
                SCOPE,
                table,
                &mut native,
                cancelled,
            )?,
        }
    };

    let mutation_like_mode = {
        let mut native = native_pool
            .try_borrow_mut()
            .map_err(|_| Error::Native(InspectError::Unsupported))?;
        sql::native_mutation_query_mode(
            &image,
            parsed.query(),
            &mut native,
            &mut query_work,
            cancelled,
        )?
        .and_then(crate::query::like_mode::sql_view)
    };

    let source =
        NativeSource::new_with_shared_budget(&mut document.source, SCOPE, &native_pool, cancelled)?;
    let comparison_pool = RefCell::new(comparison_budget());
    let (mut source, resolver) =
        source.with_deferred_database_collation_resolver(&comparison_pool)?;

    let plan = if joined_update.is_some() {
        let editor = {
            let mut native = native_pool
                .try_borrow_mut()
                .map_err(|_| Error::Native(InspectError::Unsupported))?;
            NativeJournalEditor::new(image, table, &mut native, cancelled)
                .map_err(sql::RecoverableError::Editor)?
        };
        // Read-source callbacks borrow this same native pool. Finish only after
        // complete joined staging has returned and released those borrows.
        let staged = sql::prepare_native_joined_update(
            editor,
            parsed.query(),
            &source,
            &mut query_work,
            sql::NativeMutationOptions {
                number_format: formats.number,
                date_format: formats.date,
                like_mode: mutation_like_mode,
                parameters: &bindings,
                collation: resolver.default_collation(),
                resolver: &resolver,
            },
            cancelled,
        )?;
        let mut native = native_pool
            .try_borrow_mut()
            .map_err(|_| Error::Native(InspectError::Unsupported))?;
        Plan::Update(
            staged
                .finish(&mut native, cancelled)
                .map_err(sql::RecoverableError::Plan)?,
        )
    } else if read_source_update {
        let editor = {
            let mut native = native_pool
                .try_borrow_mut()
                .map_err(|_| Error::Native(InspectError::Unsupported))?;
            NativeJournalEditor::new(image, table, &mut native, cancelled)
                .map_err(sql::RecoverableError::Editor)?
        };
        // Correlated predicates borrow this same native pool through Source.
        // Keep the consuming writer finish outside every read callback borrow.
        let staged = sql::prepare_native_read_source_update(
            editor,
            parsed.query(),
            &source,
            &mut query_work,
            sql::NativeMutationOptions {
                number_format: formats.number,
                date_format: formats.date,
                like_mode: mutation_like_mode,
                parameters: &bindings,
                collation: resolver.default_collation(),
                resolver: &resolver,
            },
            cancelled,
        )?;
        let mut native = native_pool
            .try_borrow_mut()
            .map_err(|_| Error::Native(InspectError::Unsupported))?;
        Plan::Update(
            staged
                .finish(&mut native, cancelled)
                .map_err(sql::RecoverableError::Plan)?,
        )
    } else if joined_delete.is_some() {
        let editor = {
            let mut native = native_pool
                .try_borrow_mut()
                .map_err(|_| Error::Native(InspectError::Unsupported))?;
            NativeJournalEditor::new(image, table, &mut native, cancelled)
                .map_err(sql::RecoverableDeleteError::Editor)?
        };
        // Joins use the same immutable original and shared native pool as the
        // target editor. Finish only after all read callbacks released the pool.
        let staged = sql::prepare_native_joined_delete(
            editor,
            parsed.query(),
            &source,
            &mut query_work,
            sql::NativeMutationOptions {
                number_format: formats.number,
                date_format: formats.date,
                like_mode: mutation_like_mode,
                parameters: &bindings,
                collation: resolver.default_collation(),
                resolver: &resolver,
            },
            cancelled,
        )?;
        let mut native = native_pool
            .try_borrow_mut()
            .map_err(|_| Error::Native(InspectError::Unsupported))?;
        Plan::Delete(
            staged
                .finish(&mut native, cancelled)
                .map_err(sql::RecoverableDeleteError::Plan)?,
        )
    } else if read_source_delete {
        let editor = {
            let mut native = native_pool
                .try_borrow_mut()
                .map_err(|_| Error::Native(InspectError::Unsupported))?;
            NativeJournalEditor::new(image, table, &mut native, cancelled)
                .map_err(sql::RecoverableDeleteError::Editor)?
        };
        // Predicate reads borrow this pool through NativeSource. Retain no native
        // budget borrow until complete original-handle selection has returned.
        let staged = sql::prepare_native_read_source_delete(
            editor,
            parsed.query(),
            &source,
            &mut query_work,
            sql::NativeMutationOptions {
                number_format: formats.number,
                date_format: formats.date,
                like_mode: mutation_like_mode,
                parameters: &bindings,
                collation: resolver.default_collation(),
                resolver: &resolver,
            },
            cancelled,
        )?;
        let mut native = native_pool
            .try_borrow_mut()
            .map_err(|_| Error::Native(InspectError::Unsupported))?;
        Plan::Delete(
            staged
                .finish(&mut native, cancelled)
                .map_err(sql::RecoverableDeleteError::Plan)?,
        )
    } else if let Kind::Insert(environment) = kind
        && select_insert
    {
        let inserter = {
            let mut native = native_pool
                .try_borrow_mut()
                .map_err(|_| Error::Native(InspectError::Unsupported))?;
            NativeJournalInserter::new(image, table, &mut native, cancelled)
                .map_err(sql::RecoverableInsertError::Editor)?
        };
        let context = Context::new(environment);
        let like_mode = crate::query::like_mode::sql_view(source.database_query_mode()?);

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
            Some(environment.clock),
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
            number_format: formats.number,
            date_format: formats.date,
            like_mode: mutation_like_mode,
            parameters: &bindings,
            collation: resolver.default_collation(),
            resolver: &resolver,
        };
        match kind {
            Kind::Update(_) => Plan::Update(sql::plan_recoverable_batch_update_with_options(
                image,
                parsed.query(),
                &mut native,
                &mut query_work,
                options,
                cancelled,
            )?),
            Kind::Delete(_) => Plan::Delete(sql::plan_recoverable_delete_with_options(
                image,
                parsed.query(),
                &mut native,
                &mut query_work,
                options,
                cancelled,
            )?),
            Kind::Insert(environment) => Plan::Insert(sql::plan_recoverable_insert_with_options(
                image,
                parsed.query(),
                &mut native,
                &mut query_work,
                options,
                Some(environment.clock),
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
            // NO_CLONE: The capability or reopen witness outlives this borrowed Document call
            // and owns its source path; the live Document keeps its own path.
            path: document.edits.path.clone(),
            document_revision: document.revision,
            table,
        },
        plan,
        valid: Arc::clone(&document.edits.valid),
    })
}
