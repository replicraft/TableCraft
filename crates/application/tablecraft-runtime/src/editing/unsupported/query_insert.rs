//! Unsupported hosts retain the public review boundary without native save authority.
use crate::editing::query_action::QueryActionTarget;
use crate::query::context::Environment;
use crate::{
    Error, InspectedQuery, ParameterInput, Response, display::Presentation, document::Document,
    editing::EditError,
};

pub(in crate::editing) struct State {
    pub(in crate::editing) target: QueryActionTarget,
}

pub(in crate::editing) fn review(
    _document: &mut Document,
    _target: &InspectedQuery,
    _inputs: &[ParameterInput],
    _environment: Environment<'_>,
    _output: &mut Presentation<'_>,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}
