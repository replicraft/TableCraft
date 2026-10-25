//! Every stage uses the existing retained input service under its lexical owner.
use super::{InputState, Specification, Stage};
use crate::{
    QueryError,
    execution::{Runtime, exact_keys::ExactKeys},
};

impl Runtime<'_, '_> {
    pub(super) fn progressive_stage<'p>(
        &mut self,
        specification: Specification<'p>,
        lookup: Option<ExactKeys>,
    ) -> Result<Stage<'p>, QueryError> {
        let state = self.with_progressive_owner(
            specification.read.owner,
            |runtime| -> Result<_, QueryError> {
                let output = specification.output();
                if !output.select.group.is_empty() && lookup.is_none() {
                    Ok(InputState::Text(runtime.text_group_input(output)?))
                } else if output.select.having.is_some() {
                    let (expressions, count_only) = runtime.having_operations(output)?;
                    Ok(InputState::Having(runtime.having_input(
                        output,
                        lookup,
                        expressions,
                        count_only,
                    )?))
                } else if !output.select.group.is_empty() {
                    Ok(InputState::Exact(runtime.group_input(
                        output.select,
                        lookup.ok_or(QueryError::Source)?,
                    )?))
                } else if output.select.aggregate {
                    Ok(InputState::Global(runtime.global_input(output)?))
                } else {
                    Ok(InputState::Row(0))
                }
            },
        )?;
        Ok(Stage {
            specification,
            state: Some(state),
        })
    }
}
