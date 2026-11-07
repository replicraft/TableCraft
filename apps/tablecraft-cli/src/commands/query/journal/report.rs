use super::Error;
use std::{
    fmt::Write as _,
    io::{self, Write},
};
use tablecraft_access::editing::offline::Receipt;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::journal::{
    RecoveryOutcome,
    filesystem_contracts::{CleanupDisposition, RetirementGuarantee},
};

const REPORT_CAPACITY: usize = 256;

/// Actual receipt facts, formatted using static text without terminal allocation.
pub(super) struct NamespaceFacts {
    pub(super) retirement: Option<RetirementGuarantee>,
    pub(super) cleanup: Option<CleanupDisposition>,
}

impl NamespaceFacts {
    pub(super) fn from_receipt(receipt: Receipt) -> Self {
        Self {
            retirement: receipt.retirement(),
            cleanup: receipt.cleanup(),
        }
    }

    pub(super) fn publish(self, writer: &mut dyn Write) -> io::Result<()> {
        writer.write_all(match self.retirement {
            Some(RetirementGuarantee::CapturedParentBarrier) => {
                b"Journal namespace\tRetired\nRetirement guarantee\tCaptured-parent barrier\n"
            }
            Some(RetirementGuarantee::NtfsWriteThroughRename) => {
                b"Journal namespace\tRetired\nRetirement guarantee\tNTFS write-through rename\n"
            }
            None => b"Journal namespace\tNot established\nRetirement guarantee\tNot established\n",
        })?;
        writer.write_all(match self.cleanup {
            Some(CleanupDisposition::Removed) => b"Journal cleanup\tRemoved\n",
            Some(CleanupDisposition::RetainedArtifact) => {
                b"Journal cleanup\tPrivate recovery artifact retained\n"
            }
            None => b"Journal cleanup\tNot established\n",
        })
    }
}

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

        Ok(Self {
            committed,
            rolled_back: String::new(),
        })
    }

    pub(super) fn delete(
        matched: usize,
        deleted: usize,
        budget: &mut ResourceBudget,
    ) -> Result<Self, InspectError> {
        let mut committed = buffer(budget)?;
        writeln!(committed, "Transaction\tCommitted").map_err(|_| InspectError::Allocation)?;
        writeln!(committed, "Matched rows\t{matched}").map_err(|_| InspectError::Allocation)?;
        writeln!(committed, "Deleted rows\t{deleted}").map_err(|_| InspectError::Allocation)?;

        Ok(Self {
            committed,
            rolled_back: String::new(),
        })
    }

    pub(super) fn insert(
        inserted: usize,
        budget: &mut ResourceBudget,
    ) -> Result<Self, InspectError> {
        let mut committed = buffer(budget)?;
        writeln!(committed, "Transaction\tCommitted").map_err(|_| InspectError::Allocation)?;
        writeln!(committed, "Inserted rows\t{inserted}").map_err(|_| InspectError::Allocation)?;

        Ok(Self {
            committed,
            rolled_back: String::new(),
        })
    }

    pub(super) fn recovery(budget: &mut ResourceBudget) -> Result<Self, InspectError> {
        let mut committed = buffer(budget)?;
        committed.push_str("Recovery\tCommitted\n");

        let mut rolled_back = buffer(budget)?;
        rolled_back.push_str("Recovery\tRolledBack\n");

        Ok(Self {
            committed,
            rolled_back,
        })
    }

    pub(super) fn publish(
        self,
        outcome: RecoveryOutcome,
        namespace: NamespaceFacts,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        let text = match outcome {
            RecoveryOutcome::Committed => self.committed,
            RecoveryOutcome::RolledBack => self.rolled_back,
        };
        writer
            .write_all(text.as_bytes())
            .and_then(|()| namespace.publish(writer))
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
