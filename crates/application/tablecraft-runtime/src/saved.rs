//! Revision-bound inert saved-query inspection and explicitly requested execution.
mod parameters;

use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
};
use tablecraft_access::{NativeSource, SavedRead};
use tablecraft_sql::ParseLimits;

/// Host input envelope; native reconstruction may impose a smaller declaration cap.
pub const MAX_SAVED_QUERY_PARAMETERS: usize = 256;
/// Cumulative UTF-8 bytes of input names, kinds and text, including NULL inputs.
pub const MAX_SAVED_QUERY_INPUT_BYTES: usize = 1024 * 1024;

/// Native query identity belongs to one worker's retained document revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedQueryTarget {
    pub document_revision: u64,
    pub object_id: u128,
    pub name: String,
}

/// An inert catalog declaration; presence does not establish executable support.
pub struct SavedQuery {
    pub name: String,
    pub object_id: u128,
    pub system: bool,
}

/// Declared prompt metadata. Inspection does not establish execution readiness.
pub struct ParameterSpec {
    pub name: String,
    /// Stable input token, or value when a concrete carrier must be selected.
    pub kind: String,
    /// Original native declaration rendered by the syntax owner.
    pub label: String,
    pub nullable: bool,
}

/// Explicit literal input, never SQL or an expression to evaluate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterInput {
    pub name: String,
    pub kind: String,
    pub text: String,
    pub is_null: bool,
}

impl Document {
    pub(super) fn inspect_saved_query(
        &mut self,
        target: &SavedQueryTarget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        require_revision(self, target, cancelled)?;
        let mut source = NativeSource::new(&mut self.source, SCOPE, budget(), cancelled)?;
        let saved = source.read_saved_query(&target.name, ParseLimits::default())?;
        let mut output = Presentation::new(cancelled);
        require_object(&saved, target, &mut output)?;
        let parameters = parameters::specs(saved.query(), &mut output)?;
        let target = copy_target(target, &mut output)?;
        Ok(Response::SavedQuery { target, parameters })
    }

    pub(super) fn run_saved_query(
        &mut self,
        target: &SavedQueryTarget,
        inputs: &[ParameterInput],
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        require_revision(self, target, cancelled)?;
        let mut output = Presentation::new(cancelled);
        parameters::validate_input_limits(inputs, &mut output)?;
        let mut source = NativeSource::new(&mut self.source, SCOPE, budget(), cancelled)?;
        let saved = source.read_saved_query(&target.name, ParseLimits::default())?;
        require_object(&saved, target, &mut output)?;
        let bindings = parameters::bindings(saved.query(), inputs, &mut output)?;
        crate::query::execute(saved.query(), &source, &bindings, &target.name, cancelled)
    }
}

fn require_revision(
    document: &Document,
    target: &SavedQueryTarget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(cancelled)?;
    if document.revision != target.document_revision {
        return Err(Error::StaleSavedQuery);
    }
    Ok(())
}

fn require_object(
    saved: &SavedRead,
    target: &SavedQueryTarget,
    output: &mut Presentation<'_>,
) -> Result<(), Error> {
    let object = saved.definition().object();
    output.work(1 + object.name().as_str().len() as u64 + target.name.len() as u64)?;
    if object.id().as_u128() != target.object_id || object.name().as_str() != target.name {
        return Err(Error::StaleSavedQuery);
    }
    Ok(())
}

fn copy_target(
    target: &SavedQueryTarget,
    output: &mut Presentation<'_>,
) -> Result<SavedQueryTarget, Error> {
    Ok(SavedQueryTarget {
        document_revision: target.document_revision,
        object_id: target.object_id,
        name: output.text(&target.name)?,
    })
}
