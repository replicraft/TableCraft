//! Terminal fixed-carrier DISTINCT retains first rows until successful logical EOF.
mod profile;

use super::{StreamError, StreamSummary, plan::ProgressiveArm};
use crate::{
    OutputType, QueryError,
    execution::{Runtime, cell::CellValue},
    external_sources::ExternalBinding,
};
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(super) fn progressive_distinct_rows<E>(
        &mut self,
        arm: &mut ProgressiveArm<'_>,
        union_types: Option<&[OutputType]>,
        bindings: &[ExternalBinding],
        summary: &mut StreamSummary,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), E>,
    ) -> Result<(), StreamError<E>> {
        let read = arm.read;
        let types = read.active_types(self.options.declared_integral);
        let mut lookup = Some(
            self.exact_output_keys(types)?
                .ok_or(QueryError::Unsupported)?,
        );
        let mut retained: Vec<Vec<CellValue>> = Vec::new();
        let mut cohort = self.progressive_key_cohort(types.len())?;
        let mut collect = |runtime: &mut Self,
                           cells: Vec<CellValue>,
                           _: &mut StreamSummary|
         -> Result<(), StreamError<E>> {
            runtime.validate_distinct_row(&cells, types)?;
            runtime.observe_progressive_keys(&mut cohort, &cells)?;
            let (representative, fingerprint) =
                runtime.exact_key_match(&mut lookup, &cells, &retained, Vec::as_slice, &[])?;
            if representative.is_none() {
                let position = retained.len();
                runtime.push(&mut retained, cells)?;
                if let (Some(lookup), Some(fingerprint)) = (&mut lookup, fingerprint) {
                    lookup.insert(
                        fingerprint,
                        position,
                        runtime.budget,
                        runtime.options.cancelled,
                    )?;
                }
            }
            Ok(())
        };
        self.progressive_staged_cells(arm, bindings, summary, &mut collect)?;

        // Numeric and NULL equality do not depend on text policy. Preserve the
        // complete population's provenance for later UNION conversion/consumers.
        for cells in &mut retained {
            self.complete_progressive_key_cells(cells, &cohort)?;
        }
        for (position, cells) in retained.into_iter().enumerate() {
            self.tick()?;
            if read
                .top
                .is_some_and(|(count, _)| position as u64 >= u64::from(count))
            {
                break;
            }
            self.with_progressive_owner(read.owner, |runtime| {
                runtime.progressive_publish_read(read, cells, union_types, summary, visitor)
            })?;
        }
        Ok(())
    }
}
