use super::Error;
use std::{fmt::Write as _, io::Write};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::journal::RecoveryOutcome;

const REPORT_CAPACITY: usize = 256;
const RETIRED: &str = "Journal namespace\tRetired and synchronized\n";

/// All result allocation/formatting completes before target mutation. The
/// success text is only written after known completion and lifecycle release.
pub(super) struct Reports {
    committed: String,
    rolled_back: String,
}

impl Reports {
    pub(super) fn update(
        matched: usize,
        changed: usize,
        budget: &mut ResourceBudget,
    ) -> Result<Self, InspectError> {
        let mut committed = buffer(budget)?;
        writeln!(committed, "Transaction\tCommitted").map_err(|_| InspectError::Allocation)?;
        writeln!(committed, "Matched rows\t{matched}").map_err(|_| InspectError::Allocation)?;
        writeln!(committed, "Changed rows\t{changed}").map_err(|_| InspectError::Allocation)?;
        committed.push_str(RETIRED);

        Ok(Self {
            committed,
            rolled_back: String::new(),
        })
    }

    pub(super) fn recovery(budget: &mut ResourceBudget) -> Result<Self, InspectError> {
        let mut committed = buffer(budget)?;
        committed.push_str("Recovery\tCommitted\n");
        committed.push_str(RETIRED);

        let mut rolled_back = buffer(budget)?;
        rolled_back.push_str("Recovery\tRolledBack\n");
        rolled_back.push_str(RETIRED);

        Ok(Self {
            committed,
            rolled_back,
        })
    }

    pub(super) fn publish(
        self,
        outcome: RecoveryOutcome,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        let text = match outcome {
            RecoveryOutcome::Committed => self.committed,
            RecoveryOutcome::RolledBack => self.rolled_back,
        };
        writer
            .write_all(text.as_bytes())
            .and_then(|()| writer.flush())
            .map_err(|error| Error::Output { outcome, error })
    }
}

pub(super) fn buffer(budget: &mut ResourceBudget) -> Result<String, InspectError> {
    budget.charge_allocation(REPORT_CAPACITY as u64, 1)?;
    budget.charge(Resource::Steps, REPORT_CAPACITY as u64)?;

    let mut report = String::new();
    report
        .try_reserve_exact(REPORT_CAPACITY)
        .map_err(|_| InspectError::Allocation)?;
    Ok(report)
}
