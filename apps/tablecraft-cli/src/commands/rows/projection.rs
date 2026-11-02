//! Shared explicit column selection for buffered and progressive row consumers.
use super::Args;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::ResourceBudget;

const MAX_SELECTED_COLUMNS: usize = 256;

impl Args {
    pub(super) fn projection_fits(&self) -> bool {
        self.columns.len() <= MAX_SELECTED_COLUMNS
    }

    pub(super) fn projection_names(
        &self,
        budget: &mut ResourceBudget,
    ) -> Result<Vec<&str>, InspectError> {
        if !self.projection_fits() {
            return Err(InspectError::StructureLimit);
        }

        budget.charge_allocation(self.columns.len() as u64, size_of::<&str>() as u64)?;
        let mut names = Vec::new();
        names
            .try_reserve_exact(self.columns.len())
            .map_err(|_| InspectError::Allocation)?;
        names.extend(self.columns.iter().map(String::as_str));

        Ok(names)
    }
}
