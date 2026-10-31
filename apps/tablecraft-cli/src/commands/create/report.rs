use super::Error;
use std::{fmt::Write as _, io::Write};
use tablecraft_access::creation::{CreationReceipt, PreparedCreation};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

const REPORT_BYTES: usize = 256;

pub(super) struct Report(String);

impl Report {
    pub(super) fn prepare(
        creation: &PreparedCreation,
        budget: &mut ResourceBudget,
    ) -> Result<Self, InspectError> {
        budget.charge_allocation(REPORT_BYTES as u64, 1)?;
        budget.charge(Resource::Steps, REPORT_BYTES as u64)?;
        let mut report = String::new();
        report
            .try_reserve_exact(REPORT_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        report.push_str("Database\tCreated ACE 2016\n");
        writeln!(report, "Columns\t{}", creation.column_count())
            .map_err(|_| InspectError::Allocation)?;
        writeln!(report, "Rows\t{}", creation.row_count()).map_err(|_| InspectError::Allocation)?;
        report.push_str("Durable output\tFile and captured-directory barriers completed\n");
        Ok(Self(report))
    }

    pub(super) fn publish(
        self,
        receipt: CreationReceipt,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        writer
            .write_all(self.0.as_bytes())
            .and_then(|()| writer.flush())
            .map_err(|cause| Error::Output { receipt, cause })
    }
}
