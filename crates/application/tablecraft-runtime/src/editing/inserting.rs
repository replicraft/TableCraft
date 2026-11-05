//! One reviewed row of original presence-bearing native INSERT intent.
#[cfg(any(target_os = "macos", target_os = "linux"))]
use super::EditError;
use super::{EditInput, InputHint};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use bytes::Bytes;
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use tablecraft_access::editing::journal::{NativeJournalInsertPlan, NativeJournalInserter};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use tablecraft_ace::NativeImage;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use tablecraft_db::{InsertValue, InsertableTable, OmittingInsertableTable};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use tablecraft_schema::Column;

#[cfg(any(target_os = "macos", target_os = "linux"))]
const MAX_INSERT_FIELDS: usize = 256;
#[cfg(any(target_os = "macos", target_os = "linux"))]
const MAX_INSERT_INPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub struct InsertTarget {
    pub document_revision: u64,
    pub table: String,
}

#[derive(PartialEq, Eq)]
pub enum InsertInput {
    Omitted,
    Value(EditInput),
}

pub struct InsertField {
    pub name: String,
    pub input_hint: InputHint,
    /// Reader nullability does not establish native Required/default semantics.
    pub nullable: bool,
}

pub struct InsertDraft {
    pub id: u64,
    pub target: InsertTarget,
    pub fields: Vec<InsertField>,
}

pub struct InsertReview {
    pub id: u64,
    pub draft_id: u64,
    pub target: InsertTarget,
    /// Exact user intent; omissions are not substituted with generated values.
    pub inputs: Vec<InsertInput>,
    pub inserted_rows: usize,
}

pub struct ReviewedInsert {
    pub(super) state: Box<ReviewedInsertState>,
}

pub(super) struct ReviewedInsertState {
    pub(super) path: PathBuf,
    pub(super) target: InsertTarget,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(super) original: Bytes,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(super) values: Vec<InsertValue>,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(super) plan: NativeJournalInsertPlan,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(super) valid: Arc<AtomicBool>,
}

impl ReviewedInsert {
    pub fn source_path(&self) -> &Path {
        &self.state.path
    }

    pub fn target(&self) -> &InsertTarget {
        &self.state.target
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
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
        let image = NativeImage::capture(&mut self.source, SCOPE, &mut work, cancelled)?;
        let inserter = NativeJournalInserter::new(image, &target.table, &mut work, cancelled)
            .map_err(EditError::from)?;
        let columns = inserter.columns();
        if columns.is_empty() || columns.len() > MAX_INSERT_FIELDS {
            return Err(EditError::Unavailable.into());
        }
        let mut output = Presentation::new(cancelled);
        output.allocation(columns.len(), size_of::<InsertField>())?;
        let mut fields = Vec::new();
        fields.try_reserve_exact(columns.len())?;
        for column in columns {
            fields.push(InsertField {
                name: output.text(column.name())?,
                input_hint: InputHint::from(column.data_type()),
                nullable: column.nullable(),
            });
        }
        let id = super::ids::issue_id()?;
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
        check_input_limits(inputs, cancelled)?;
        let mut output = Presentation::new(cancelled);
        let target = copy_target(target, &mut output)?;
        let mut work = budget();
        let image = NativeImage::capture(&mut self.source, SCOPE, &mut work, cancelled)?;
        let inserter = NativeJournalInserter::new(image, &target.table, &mut work, cancelled)
            .map_err(EditError::from)?;
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
        output.allocation(1, size_of::<ReviewedInsertState>())?;
        let capability = ReviewedInsert {
            state: Box::new(ReviewedInsertState {
                path: self.edits.path.clone(),
                original: self.edits.original.clone(),
                target,
                values: retained_values,
                plan,
                valid: Arc::clone(&self.edits.valid),
            }),
        };
        Ok(Response::InsertReviewed {
            review: InsertReview {
                id: super::ids::issue_id()?,
                draft_id,
                target: review_target,
                inputs,
                inserted_rows: 1,
            },
            capability,
        })
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn copy_target(
    target: &InsertTarget,
    output: &mut Presentation<'_>,
) -> Result<InsertTarget, Error> {
    Ok(InsertTarget {
        document_revision: target.document_revision,
        table: output.text(&target.table)?,
    })
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn check_input_limits(inputs: &[InsertInput], cancelled: &dyn Fn() -> bool) -> Result<(), Error> {
    if inputs.is_empty() || inputs.len() > MAX_INSERT_FIELDS {
        return Err(EditError::InputLimit.into());
    }
    let mut bytes = 0_usize;
    for input in inputs {
        check(cancelled)?;
        if let InsertInput::Value(input) = input {
            if input.text.len() > crate::scalar_input::MAX_INPUT_BYTES {
                return Err(EditError::InputLimit.into());
            }
            bytes = bytes
                .checked_add(input.text.len())
                .ok_or(EditError::InputLimit)?;
            if bytes > MAX_INSERT_INPUT_BYTES {
                return Err(EditError::InputLimit.into());
            }
        }
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
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
            InsertInput::Value(input) => InsertValue::Supplied(crate::scalar_input::parse(
                column.data_type(),
                &input.text,
                input.is_null,
                output,
            )?),
        });
    }
    Ok(values)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
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
