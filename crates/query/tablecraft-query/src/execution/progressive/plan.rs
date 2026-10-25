//! Preflight every selected arm before any progressive record callback.
use super::{
    super::{Runtime, exact_keys::ExactKeys},
    input::Input,
};
use crate::{
    OutputType, QueryError,
    external_sources::ExternalBinding,
    plan::{Expr, PreparedBody, ReadNode, ReadPlan, SelectPlan},
};

pub(super) struct ProgressiveArm<'p> {
    pub(super) read: &'p ReadPlan,
    pub(super) select: &'p SelectPlan,
    pub(super) aggregates: Vec<&'p Expr>,
    pub(super) group_lookup: Option<ExactKeys>,
    pub(super) input: Input<'p>,
}

pub(super) struct ProgressivePlan<'p> {
    pub(super) arms: Vec<ProgressiveArm<'p>>,
    pub(super) union_types: Option<&'p [OutputType]>,
    pub(super) bindings: &'p [ExternalBinding],
}

impl Runtime<'_, '_> {
    pub(super) fn progressive_plan<'p>(
        &mut self,
        body: &'p PreparedBody,
        bindings: &'p [ExternalBinding],
    ) -> Result<ProgressivePlan<'p>, QueryError> {
        self.tick()?;
        let PreparedBody::Read(read) = body else {
            return Err(QueryError::Unsupported);
        };
        let count = match &read.node {
            ReadNode::Union { rest, .. } => rest
                .len()
                .checked_add(1)
                .ok_or(QueryError::StructureLimit)?,
            _ => 1,
        };
        self.budget
            .charge_allocation(count as u64, size_of::<ProgressiveArm<'_>>() as u64)?;
        let mut arms = Vec::new();
        arms.try_reserve_exact(count)
            .map_err(|_| QueryError::Allocation)?;

        let union_types = if let ReadNode::Union { first, rest } = &read.node {
            if !read.sort.is_empty() || read.distinct || read.top.is_some() {
                return Err(QueryError::Unsupported);
            }
            // Admission of the entire shape precedes metadata validation and
            // source activation, including later empty/filtered arms.
            self.progressive_arm(first, &mut arms)?;
            for (all, arm) in rest {
                self.tick()?;
                if !all {
                    return Err(QueryError::Unsupported);
                }
                self.progressive_arm(arm, &mut arms)?;
            }
            Some(read.active_types(self.options.declared_integral))
        } else {
            self.progressive_arm(read, &mut arms)?;
            None
        };

        for arm in &arms {
            self.check_table_schema(&arm.input.leaf.node)?;
        }
        self.tick()?;
        Ok(ProgressivePlan {
            arms,
            union_types,
            bindings,
        })
    }

    fn progressive_arm<'p>(
        &mut self,
        read: &'p ReadPlan,
        arms: &mut Vec<ProgressiveArm<'p>>,
    ) -> Result<(), QueryError> {
        let (select, aggregates) = self.progressive_select(read)?;
        let input = self.progressive_input(&select.source)?;
        let group_lookup = self.progressive_group_lookup(select)?;
        arms.push(ProgressiveArm {
            read,
            select,
            aggregates,
            group_lookup,
            input,
        });
        Ok(())
    }

    pub(super) fn progressive_group_lookup(
        &mut self,
        select: &SelectPlan,
    ) -> Result<Option<ExactKeys>, QueryError> {
        if select.group.is_empty() {
            return Ok(None);
        }
        if select.group_types.len() != select.group.len() {
            return Err(QueryError::Width);
        }
        let lookup = self.exact_profiled_keys(&select.group_types)?;
        if lookup.is_none() && !self.progressive_text_profile(&select.group_types)? {
            return Err(QueryError::Unsupported);
        }
        Ok(lookup)
    }
}
