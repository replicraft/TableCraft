//! Shared host policies live for one buffered or progressive query operation.
use super::{Error, retrieval::Controls};
use tablecraft_expressions::{
    comparison::Collation,
    functions::{
        error_messages::EnglishErrorMessages, math::random::RndState, session::AccessSessionContext,
    },
};
use tablecraft_query::{ExecutionOptions, QueryError, source::CollationResolver};

pub(super) fn with_context<T, E: From<Error>>(
    controls: &Controls,
    collation: Option<&dyn Collation>,
    resolver: Option<&dyn CollationResolver>,
    operation: impl FnOnce(ExecutionOptions<'_>) -> Result<T, E>,
) -> Result<T, E> {
    let text_policy = controls
        .text_policy
        .build(collation, controls.comparison_limits.budget(), &|| false)
        .map_err(|error| Error::Retrieval(QueryError::Evaluation(error)))?;
    let collation = text_policy
        .as_ref()
        .map(|provider| provider as &dyn Collation)
        .or(collation);
    let clock = super::clock::HostClock::new(controls.clock);
    let error_messages = EnglishErrorMessages::default();
    let random = RndState::new(controls.random_seed).map_err(|_| Error::Parameter)?;

    operation(ExecutionOptions {
        declared_integral: controls.declared_integral,
        max_text_bytes: controls.max_text_bytes,
        limits: controls.execution_limits,
        error_messages: Some(&error_messages),
        access_session: Some(AccessSessionContext::unsecured()),
        case_locale: controls.case_locale.as_ref(),
        clock: Some(&clock),
        random: Some(&random),
        year_window: controls.year_window,
        character_code_page: controls.character_code_page,
        number_format: controls.number_format,
        date_format: controls.date_format,
        reference_year: controls.reference_year,
        first_day_of_week: controls.first_day_of_week,
        first_week_of_year: controls.first_week_of_year,
        like_mode: controls.like_mode.map(Into::into),
        collation,
        collation_resolver: resolver,
        ..Default::default()
    })
}
