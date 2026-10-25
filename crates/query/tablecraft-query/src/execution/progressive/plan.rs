//! Preflight every selected arm before any progressive record callback.
use super::super::Runtime;
use crate::{
    OutputType, QueryError,
    external_sources::SourceNamespace,
    plan::{Expr, PreparedBody, ReadNode, ReadPlan, SelectPlan},
};

pub(super) struct ProgressiveArm<'p> {
    pub(super) read: &'p ReadPlan,
    pub(super) select: &'p SelectPlan,
    pub(super) counts: Vec<&'p Expr>,
}

pub(super) struct ProgressivePlan<'p> {
    pub(super) arms: Vec<ProgressiveArm<'p>>,
    pub(super) union_types: Option<&'p [OutputType]>,
}

impl Runtime<'_, '_> {
    pub(super) fn progressive_plan<'p>(
        &mut self,
        body: &'p PreparedBody,
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
            if read.owner != SourceNamespace::Root
                || !read.sort.is_empty()
                || read.distinct
                || read.top.is_some()
            {
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
            self.check_table_schema(&arm.select.source.node)?;
        }
        self.tick()?;
        Ok(ProgressivePlan { arms, union_types })
    }

    fn progressive_arm<'p>(
        &mut self,
        read: &'p ReadPlan,
        arms: &mut Vec<ProgressiveArm<'p>>,
    ) -> Result<(), QueryError> {
        let (select, counts) = self.progressive_select(read)?;
        arms.push(ProgressiveArm {
            read,
            select,
            counts,
        });
        Ok(())
    }
}
