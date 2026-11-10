//! Native insertion review and its source-bound private capability.
use crate::editing::InsertRetryWitness;
use crate::editing::{
    InsertDraft, InsertField, InsertInput, InsertReview, InsertTarget, ReviewedInsert,
};
use crate::scalar_input::InputHint;
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::{EditError, EditInput},
};
use bytes::Bytes;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tablecraft_access::editing::journal::{NativeJournalInsertPlan, NativeJournalInserter};
use tablecraft_ace::{InspectError, TableFeatures, inspect_table_features};
use tablecraft_db::{InsertValue, InsertableTable, OmittingInsertableTable};
use tablecraft_schema::Column;

pub(in crate::editing) struct State {
    pub(in crate::editing) path: PathBuf,
    pub(in crate::editing) target: InsertTarget,
    pub(in crate::editing) retry: Arc<InsertRetryWitness>,
    pub(in crate::editing) original: Bytes,
    pub(in crate::editing) values: Vec<InsertValue>,
    pub(in crate::editing) plan: NativeJournalInsertPlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

impl Document {
    pub(crate) fn begin_insert(
        &mut self,
        target: &InsertTarget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if target.document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        if !self.source.editable_native() {
            return Err(EditError::ReadOnlySource.into());
        }
        let mut work = budget();
        let features =
            inspect_table_features(&mut self.source, SCOPE, &target.table, &mut work, cancelled)?;
        // NO_CLONE: NativeImage owns the checked source while Document keeps using it; this
        // shares immutable bytes instead of copying the database image.
        let image = NativeJournalInserter::capture_shared_image(
            self.edits.image_source.clone(),
            SCOPE,
            &target.table,
            &mut work,
            cancelled,
        )
        .map_err(EditError::from)?;
        let inserter = NativeJournalInserter::new(image, &target.table, &mut work, cancelled)
            .map_err(EditError::from)?;
        let columns = inserter.columns();
        if columns.is_empty() {
            return Err(EditError::Unavailable.into());
        }
        let mut output = Presentation::new(cancelled);
        output.allocation(columns.len(), size_of::<InsertField>())?;
        let mut fields = Vec::new();
        fields.try_reserve_exact(columns.len())?;
        validate_feature_columns(&features, columns, &mut output)?;
        for (column, feature) in columns.iter().zip(features.columns()) {
            fields.push(InsertField {
                name: output.text(column.name())?,
                input_hint: InputHint::from(column.data_type()),
                nullable: column.nullable(),
                auto_number: feature.auto_number(),
            });
        }
        let id = crate::editing::platform::ids::issue_id()?;
        let retained = copy_target(target, &mut output)?;
        let target = copy_target(target, &mut output)?;
        self.edits.insert_draft = Some((id, retained));
        Ok(Response::InsertDraft(InsertDraft { id, target, fields }))
    }

    pub(crate) fn review_insert(
        &mut self,
        draft_id: u64,
        inputs: &[InsertInput],
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        let Some((current, target)) = &self.edits.insert_draft else {
            return Err(EditError::StaleDraft.into());
        };
        if *current != draft_id || target.document_revision != self.revision {
            return Err(EditError::StaleDraft.into());
        }
        if !self.edits.valid.load(Ordering::Acquire) || !self.source.editable_native() {
            return Err(EditError::StaleDraft.into());
        }
        check_input_shape(inputs, cancelled)?;
        let mut output = Presentation::new(cancelled);
        let target = copy_target(target, &mut output)?;
        let mut work = budget();
        let features =
            inspect_table_features(&mut self.source, SCOPE, &target.table, &mut work, cancelled)?;
        // NO_CLONE: NativeImage owns the checked source while Document keeps using it; this
        // shares immutable bytes instead of copying the database image.
        let image = NativeJournalInserter::capture_shared_image(
            self.edits.image_source.clone(),
            SCOPE,
            &target.table,
            &mut work,
            cancelled,
        )
        .map_err(EditError::from)?;
        let inserter = NativeJournalInserter::new(image, &target.table, &mut work, cancelled)
            .map_err(EditError::from)?;
        validate_feature_columns(&features, inserter.columns(), &mut output)?;
        require_omitted_auto_numbers(&features, inputs, &mut output)?;
        let values = parse_inputs(inserter.columns(), inputs, &mut output)?;
        let retained_values = parse_inputs(inserter.columns(), inputs, &mut output)?;
        let plan = inserter
            .plan_insert_with_omissions(values, &mut work, cancelled)
            .map_err(EditError::from)?;
        if plan.inserted_rows() != 1 {
            return Err(EditError::Unavailable.into());
        }
        let inputs = copy_inputs(inputs, &mut output)?;
        let review_target = copy_target(&target, &mut output)?;
        output.allocation(self.edits.path.as_os_str().len(), 1)?;
        output.allocation(1, size_of::<State>())?;
        output.allocation(self.edits.path.as_os_str().len(), 1)?;
        output.allocation(1, size_of::<InsertRetryWitness>())?;
        let retry = Arc::new(InsertRetryWitness {
            // NO_CLONE: The capability or reopen witness outlives this borrowed Document call
            // and owns its source path; the live Document keeps its own path.
            path: self.edits.path.clone(),
            // NO_CLONE: The reader and retained native witness need independent lifetimes;
            // Bytes shares the immutable original image rather than copying it.
            original: self.edits.original.clone(),
            target: copy_target(&target, &mut output)?,
        });
        let capability = ReviewedInsert {
            state: Box::new(State {
                // NO_CLONE: The capability or reopen witness outlives this borrowed Document
                // call and owns its source path; the live Document keeps its own
                // path.
                path: self.edits.path.clone(),
                // NO_CLONE: The reader and retained native witness need independent lifetimes;
                // Bytes shares the immutable original image rather than copying it.
                original: self.edits.original.clone(),
                target,
                retry,
                values: retained_values,
                plan,
                valid: Arc::clone(&self.edits.valid),
            }),
        };
        Ok(Response::InsertReviewed {
            review: InsertReview {
                id: crate::editing::platform::ids::issue_id()?,
                draft_id,
                target: review_target,
                inputs,
                inserted_rows: 1,
            },
            capability,
        })
    }
}

fn copy_target(
    target: &InsertTarget,
    output: &mut Presentation<'_>,
) -> Result<InsertTarget, Error> {
    Ok(InsertTarget {
        document_revision: target.document_revision,
        table: output.text(&target.table)?,
    })
}

fn check_input_shape(inputs: &[InsertInput], cancelled: &dyn Fn() -> bool) -> Result<(), Error> {
    check(cancelled)?;
    if inputs.is_empty() {
        return Err(EditError::InputLimit.into());
    }
    Ok(())
}

fn validate_feature_columns(
    features: &TableFeatures,
    columns: &[Column],
    output: &mut Presentation<'_>,
) -> Result<(), Error> {
    if features.schema().columns().len() != columns.len() {
        return Err(InspectError::Malformed.into());
    }
    for (feature, column) in features.columns().zip(columns) {
        output.work(feature.column().name().len() as u64 + column.name().len() as u64)?;
        if feature.column().name() != column.name() {
            return Err(InspectError::Malformed.into());
        }
    }
    Ok(())
}

fn require_omitted_auto_numbers(
    features: &TableFeatures,
    inputs: &[InsertInput],
    output: &mut Presentation<'_>,
) -> Result<(), Error> {
    if features.schema().columns().len() != inputs.len() {
        return Err(EditError::InvalidInput.into());
    }
    for (feature, input) in features.columns().zip(inputs) {
        output.work(1)?;
        if feature.auto_number() && !matches!(input, InsertInput::Omitted) {
            return Err(EditError::InvalidInput.into());
        }
    }
    Ok(())
}

fn parse_inputs(
    columns: &[Column],
    inputs: &[InsertInput],
    output: &mut Presentation<'_>,
) -> Result<Vec<InsertValue>, Error> {
    if columns.len() != inputs.len() {
        return Err(EditError::InvalidInput.into());
    }
    output.allocation(inputs.len(), size_of::<InsertValue>())?;
    let mut values = Vec::new();
    values.try_reserve_exact(inputs.len())?;
    for (column, input) in columns.iter().zip(inputs) {
        output.work(1)?;
        values.push(match input {
            InsertInput::Omitted => InsertValue::Omitted,
            InsertInput::Value(input) => {
                InsertValue::Supplied(crate::scalar_input::parse_declared(
                    column.type_descriptor(),
                    &input.text,
                    input.is_null,
                    output,
                )?)
            }
        });
    }
    Ok(values)
}

fn copy_inputs(
    inputs: &[InsertInput],
    output: &mut Presentation<'_>,
) -> Result<Vec<InsertInput>, Error> {
    output.allocation(inputs.len(), size_of::<InsertInput>())?;
    let mut copied = Vec::new();
    copied.try_reserve_exact(inputs.len())?;
    for input in inputs {
        copied.push(match input {
            InsertInput::Omitted => InsertInput::Omitted,
            InsertInput::Value(input) => InsertInput::Value(EditInput {
                text: output.text(&input.text)?,
                is_null: input.is_null,
            }),
        });
    }
    Ok(copied)
}
