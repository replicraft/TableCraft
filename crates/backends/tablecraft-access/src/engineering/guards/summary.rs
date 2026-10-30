use super::GuardBackendError;
use tablecraft_ace::{BeforeEvent, BeforeEventGuardPreparation, BeforeGuardTrace, InspectError};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// An explicit, evaluated rejection; its text is not printed by Debug/Display.
pub struct GuardRejectionSummary {
    number: i32,
    description: Option<String>,
}

impl GuardRejectionSummary {
    /// Checked signed Long result.
    pub const fn number(&self) -> i32 {
        self.number
    }

    /// Optional explicit description; absent differs from present-empty.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Owned presentation facts after one completed engine guard preparation.
/// This never contains a candidate or a replayable write/save capability.
pub struct GuardPreparationSummary {
    event: BeforeEvent,
    selected: usize,
    programs: Vec<GuardPropertySummary>,
    trace: Vec<BeforeGuardTrace>,
    rejection: Option<GuardRejectionSummary>,
}

impl GuardPreparationSummary {
    /// Explicit selected before-event context.
    pub const fn event(&self) -> BeforeEvent {
        self.event
    }

    /// All SQL-selected intent rows, including rows after a rejecting event.
    pub const fn selected_rows(&self) -> usize {
        self.selected
    }

    /// Original native property witnesses checked by the engine.
    pub const fn program_count(&self) -> usize {
        self.programs.len()
    }

    /// Selected source properties in the caller's explicit engineering order.
    pub fn properties(&self) -> &[GuardPropertySummary] {
        &self.programs
    }

    /// Completed visited order; SQL/XML/field values are not embedded.
    pub fn trace(&self) -> &[BeforeGuardTrace] {
        &self.trace
    }

    /// Explicit evaluated rejection, if any; None certifies only this guard subset.
    pub fn rejection(&self) -> Option<&GuardRejectionSummary> {
        self.rejection.as_ref()
    }
}

/// Exact selected source identity, not detected required-macro coverage.
pub struct GuardPropertySummary {
    group: u16,
    name: String,
}

impl GuardPropertySummary {
    /// Native group kind, without semantic reinterpretation.
    pub const fn group(&self) -> u16 {
        self.group
    }

    /// Exact selected source property name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

pub(in crate::engineering) fn retain(
    report: &BeforeEventGuardPreparation<'_>,
    selected: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<GuardPreparationSummary, GuardBackendError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(
            report.trace().len() as u64,
            size_of::<BeforeGuardTrace>() as u64,
        )
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, report.trace().len() as u64)
        .map_err(InspectError::from)?;
    let mut trace = Vec::new();
    trace
        .try_reserve_exact(report.trace().len())
        .map_err(|_| InspectError::Allocation)?;
    trace.extend_from_slice(report.trace());
    budget
        .charge_allocation(
            report.programs().len() as u64,
            size_of::<GuardPropertySummary>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut programs = Vec::new();
    programs
        .try_reserve_exact(report.programs().len())
        .map_err(|_| InspectError::Allocation)?;
    for program in report.programs() {
        check(budget, cancelled)?;
        budget
            .charge_allocation(program.name().len() as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, program.name().len() as u64)
            .map_err(InspectError::from)?;
        let mut name = String::new();
        name.try_reserve_exact(program.name().len())
            .map_err(|_| InspectError::Allocation)?;
        name.push_str(program.name());
        programs.push(GuardPropertySummary {
            group: program.group(),
            name,
        });
    }
    let rejection = report
        .rejection()
        .map(|rejection| {
            let description = rejection
                .description()
                .map(|text| {
                    check(budget, cancelled)?;
                    budget
                        .charge_allocation(text.len() as u64, 1)
                        .map_err(InspectError::from)?;
                    budget
                        .charge(Resource::Steps, text.len() as u64)
                        .map_err(InspectError::from)?;
                    let mut owned = String::new();
                    owned
                        .try_reserve_exact(text.len())
                        .map_err(|_| InspectError::Allocation)?;
                    owned.push_str(text);
                    Ok::<_, GuardBackendError>(owned)
                })
                .transpose()?;
            Ok::<_, GuardBackendError>(GuardRejectionSummary {
                number: rejection.number(),
                description,
            })
        })
        .transpose()?;
    check(budget, cancelled)?;
    Ok(GuardPreparationSummary {
        event: report.event(),
        selected,
        programs,
        trace,
        rejection,
    })
}

pub(in crate::engineering) fn check(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), GuardBackendError> {
    if cancelled() {
        return Err(InspectError::Cancelled.into());
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)?;
    Ok(())
}
