//! Providers for one explicit retrieval run, independent of display preferences.
pub(crate) mod formats;
use formats::QueryExpressionFormats;
use jiff::{Timestamp, tz::TimeZone};
use std::time::SystemTime;
use tablecraft_core::values::ExtendedDateTime;
use tablecraft_expressions::{
    EvalError,
    functions::{
        dates::clock::{Clock, ClockReading},
        error_messages::EnglishErrorMessages,
        math::random::RndState,
        session::AccessSessionContext,
    },
};
use tablecraft_query::ExecutionOptions;

const NANOS_PER_CLOCK_TICK: u32 = 100;

/// System discovery is lazy and fallible. Never substitute UTC for an unavailable
/// local timezone, and never read this clock while inspecting query metadata.
pub(crate) struct LocalClock;

impl Clock for LocalClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        let zone = TimeZone::try_system().map_err(|_| EvalError::ClockUnavailable)?;
        let instant =
            Timestamp::try_from(SystemTime::now()).map_err(|_| EvalError::ClockUnavailable)?;
        let local = instant.to_zoned(zone).datetime();
        let value = ExtendedDateTime::new(
            local.year().try_into().map_err(|_| EvalError::Overflow)?,
            local.month().try_into().map_err(|_| EvalError::Overflow)?,
            local.day().try_into().map_err(|_| EvalError::Overflow)?,
            local.hour().try_into().map_err(|_| EvalError::Overflow)?,
            local.minute().try_into().map_err(|_| EvalError::Overflow)?,
            local.second().try_into().map_err(|_| EvalError::Overflow)?,
            u32::try_from(local.subsec_nanosecond()).map_err(|_| EvalError::Overflow)?
                / NANOS_PER_CLOCK_TICK,
        )
        .map_err(|_| EvalError::Overflow)?;
        ClockReading::new(value)
    }
}

/// Immutable per-worker providers borrowed for one command's execution.
#[derive(Clone, Copy)]
pub(crate) struct Environment<'a> {
    pub clock: &'a dyn Clock,
    pub formats: QueryExpressionFormats,
}

/// One statement's sequential Basic random state and immutable expression
/// providers. Nested/named reads share them. A subsequent explicit run starts
/// from the reviewed default seed, matching the existing CLI policy; this does
/// not claim Access application-session Randomize/Rnd lifecycle compatibility.
pub(crate) struct Context<'a> {
    clock: &'a dyn Clock,
    formats: QueryExpressionFormats,
    random: RndState,
    messages: EnglishErrorMessages,
}

impl<'a> Context<'a> {
    pub(crate) fn new(environment: Environment<'a>) -> Self {
        Self {
            clock: environment.clock,
            formats: environment.formats,
            random: RndState::default(),
            messages: EnglishErrorMessages::default(),
        }
    }

    pub(crate) fn options(&self) -> ExecutionOptions<'_> {
        ExecutionOptions {
            clock: Some(self.clock),
            number_format: self.formats.number,
            date_format: self.formats.date,
            random: Some(&self.random),
            error_messages: Some(&self.messages),
            // Admin is declared unsecured expression metadata, not OS identity,
            // MDW authentication, or permission to read or mutate an object.
            access_session: Some(AccessSessionContext::unsecured()),
            ..Default::default()
        }
    }
}
