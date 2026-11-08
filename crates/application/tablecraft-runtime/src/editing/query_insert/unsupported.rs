//! Unsupported hosts retain the public review boundary without native save authority.
use crate::editing::query_action::QueryActionTarget;
use crate::{
    Error, InspectedQuery, ParameterInput, Response, display::Presentation, document::Document,
    editing::EditError,
};
use tablecraft_expressions::functions::dates::clock::Clock;

pub(in crate::editing) struct State {
    pub(in crate::editing) target: QueryActionTarget,
}

pub(super) fn review(
    _document: &mut Document,
    _target: &InspectedQuery,
    _inputs: &[ParameterInput],
    _clock: &dyn Clock,
    _output: &mut Presentation<'_>,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}
