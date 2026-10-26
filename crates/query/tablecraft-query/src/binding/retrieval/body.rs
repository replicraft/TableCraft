//! Keep UNION and TABLE construction scratch out of ordinary SELECT recursion.
use super::Binder;
use crate::{
    OutputType, QueryError,
    plan::{ProfiledType, ReadNode, ReadPlan, Slot},
    profile::MAX_UNION_BRANCHES,
};
use tablecraft_sql::{
    query::{ReadBody, UnionArm},
    select::{Name, Projection, Select, SelectPredicate, SortKey, Table, TableSource},
};

struct UnionInput {
    output_types: Vec<OutputType>,
    materialized_types: Vec<OutputType>,
    declared_types: Vec<OutputType>,
    arms: Vec<(bool, ReadPlan)>,
}

impl Binder<'_> {
    #[inline(never)]
    pub(super) fn reference_body(
        &mut self,
        name: &Name,
        reference: bool,
        sort: &[SortKey],
        outer: &[&[Slot]],
        depth: usize,
    ) -> Result<ReadPlan, QueryError> {
        if reference
            && (!self.retrieval
                || self
                    .namespace_source(self.owner, depth)?
                    .query(name.as_str())?
                    .is_none())
        {
            return Err(QueryError::Unsupported);
        }
        self.select(
            &Select {
                predicate: SelectPredicate::All,
                projection: vec![Projection::Wildcard(None)],
                from: vec![TableSource::Table(Table {
                    name: name.clone(),
                    alias: None,
                    database: None,
                })],
                group_by: vec![],
                having: None,
                into: None,
                into_external: None,
                external: None,
                owner_access: false,
                selection: None,
                order_by: vec![],
            },
            sort,
            outer,
            depth + 1,
            None,
        )
    }

    #[inline(never)]
    pub(super) fn union_body(
        &mut self,
        first: &ReadBody,
        rest: &[UnionArm],
        sort: &[SortKey],
        outer: &[&[Slot]],
        depth: usize,
    ) -> Result<ReadPlan, QueryError> {
        if rest.len() > MAX_UNION_BRANCHES {
            return Err(QueryError::StructureLimit);
        }
        let first = self.body(first, &[], outer, depth + 1)?;
        let input = self.union_remaining(&first, rest, outer, depth)?;
        self.finish_union(first, input, sort, outer, depth)
    }

    // Recursive arm binding retains only the current UNION inputs. Policy,
    // output-slot and final-plan scratch belongs to the nonrecursive finisher.
    #[inline(never)]
    fn union_remaining(
        &mut self,
        first: &ReadPlan,
        rest: &[UnionArm],
        outer: &[&[Slot]],
        depth: usize,
    ) -> Result<UnionInput, QueryError> {
        let mut input = self.union_input(first)?;
        self.bind_remaining_union_arms(&mut input, first.names.len(), rest, outer, depth)?;
        Ok(input)
    }

    // Borrow accumulated state across later-arm recursion. This frame has no
    // owned UNION input or final ReadPlan return slots alongside the child plan.
    #[inline(never)]
    fn bind_remaining_union_arms(
        &mut self,
        input: &mut UnionInput,
        width: usize,
        rest: &[UnionArm],
        outer: &[&[Slot]],
        depth: usize,
    ) -> Result<(), QueryError> {
        for arm in rest {
            let plan = self.body(&arm.body, &[], outer, depth + 1)?;
            // Validate each written arm before consulting the next source.
            self.append_union_arm(input, width, arm.all, plan)?;
        }
        Ok(())
    }

    #[inline(never)]
    fn union_input(&mut self, first: &ReadPlan) -> Result<UnionInput, QueryError> {
        // Preserve the existing reservations before later-arm source callbacks.
        let output_types = self.type_vector(first.output_types.len())?;
        let mut materialized_types = self.type_vector(first.output_types.len())?;
        materialized_types.extend_from_slice(&first.materialized_types);
        let mut declared_types = self.type_vector(first.output_types.len())?;
        declared_types.extend_from_slice(&first.declared_types);
        Ok(UnionInput {
            output_types,
            materialized_types,
            declared_types,
            arms: Vec::new(),
        })
    }

    #[inline(never)]
    fn append_union_arm(
        &mut self,
        input: &mut UnionInput,
        width: usize,
        all: bool,
        plan: ReadPlan,
    ) -> Result<(), QueryError> {
        if plan.names.len() != width {
            return Err(QueryError::Width);
        }
        for (result, other) in input
            .materialized_types
            .iter_mut()
            .zip(&plan.materialized_types)
        {
            *result = result.union_result(*other);
        }
        for (result, other) in input.declared_types.iter_mut().zip(&plan.declared_types) {
            *result = result.union_result(*other);
        }
        input.arms.push((all, plan));
        Ok(())
    }

    #[inline(never)]
    fn finish_union(
        &mut self,
        first: ReadPlan,
        input: UnionInput,
        sort: &[SortKey],
        outer: &[&[Slot]],
        depth: usize,
    ) -> Result<ReadPlan, QueryError> {
        let UnionInput {
            mut output_types,
            materialized_types,
            declared_types,
            arms,
        } = input;
        for (&materialized, &declared) in materialized_types.iter().zip(&declared_types) {
            output_types.push(
                ProfiledType {
                    materialized,
                    declared,
                }
                .conservative(),
            );
        }
        let mut text_policies = self.policy_vector(first.text_policies.len())?;
        text_policies.extend_from_slice(&first.text_policies);
        for (_, arm) in &arms {
            for (policy, other) in text_policies.iter_mut().zip(&arm.text_policies) {
                *policy = policy.merge(*other);
            }
        }
        self.charge(
            first
                .names
                .len()
                .checked_mul(size_of::<Slot>())
                .ok_or(QueryError::StructureLimit)?,
        )?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(first.names.len())
            .map_err(|_| QueryError::Allocation)?;
        for (index, (name, policy)) in first.names.iter().zip(&text_policies).enumerate() {
            slots.push(Slot {
                output_type: ProfiledType {
                    materialized: materialized_types[index],
                    declared: declared_types[index],
                },
                declaration: None,
                text_policy: *policy,
                available: true,
                collection_index: None,
                member: None,
                name: self.copy_name(name)?,
                qualifier: None,
            });
        }
        let order = self.sort(sort, &[], &slots, outer, false, depth + 1)?;
        let declarations = self.unknown_declarations(output_types.len())?;
        Ok(ReadPlan {
            owner: self.owner,
            declarations,
            output_types,
            materialized_types,
            declared_types,
            text_policies,
            names: first.names.clone(),
            node: ReadNode::Union {
                first: Box::new(first),
                rest: arms,
            },
            sort: order,
            distinct: false,
            top: None,
        })
    }
}
