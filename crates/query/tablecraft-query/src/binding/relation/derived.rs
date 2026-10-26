//! Derived list outputs keep their origin and expose prunable virtual members.
use super::Binder;
use crate::{
    OutputType, QueryError,
    plan::{MemberSlot, ProfiledType, ReadPlan, Relation, RelationNode, Slot},
    profile::SOURCE_SCHEMA_POLICY,
};
use tablecraft_expressions::numeric::OperandDeclaration;

impl Binder<'_> {
    pub(in crate::binding) fn derived_relation(
        &mut self,
        read: ReadPlan,
        qualifier: Option<&str>,
    ) -> Result<Relation, QueryError> {
        let list_count = read
            .output_types
            .iter()
            .filter(|kind| kind.is_multivalue())
            .count();
        let width = read
            .names
            .len()
            .checked_add(list_count)
            .ok_or(QueryError::StructureLimit)?;
        if read.names.len() > SOURCE_SCHEMA_POLICY.max_columns {
            return Err(QueryError::StructureLimit);
        }
        self.charge(
            size_of::<ReadPlan>() + width * size_of::<Slot>() + list_count * size_of::<usize>(),
        )?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        let mut expansion = Vec::new();
        expansion
            .try_reserve_exact(list_count)
            .map_err(|_| QueryError::Allocation)?;
        let mut list = 0;
        for (index, name) in read.names.iter().enumerate() {
            self.work()?;
            let is_list = read.output_types[index].is_multivalue();
            let origin = if is_list {
                Some(
                    read.collection_origin(index)
                        .ok_or(QueryError::Unsupported)?,
                )
            } else {
                None
            };
            if let Some(origin) = origin {
                for prior in 0..index {
                    self.work()?;
                    if read.collection_origin(prior) == Some(origin)
                        && (read.output_types[prior] != read.output_types[index]
                            || read.text_policies[prior] != read.text_policies[index])
                    {
                        return Err(QueryError::Unsupported);
                    }
                }
            }
            slots.push(Slot {
                output_type: ProfiledType {
                    materialized: read.materialized_types[index],
                    declared: read.declared_types[index],
                },
                declaration: read.declarations[index],
                text_policy: read.text_policies[index],
                available: !is_list,
                collection_index: is_list.then_some(list),
                collection_origin: origin,
                retained_member: false,
                member: None,
                name: self.copy_name(name)?,
                qualifier: qualifier.map(|name| self.copy_name(name)).transpose()?,
            });
            list += usize::from(is_list);
        }
        let mut list = 0;
        for (index, kind) in read.output_types.iter().enumerate() {
            self.work()?;
            if let OutputType::Multivalue { element_type, .. } = kind {
                slots.push(Slot {
                    output_type: ProfiledType::uniform(OutputType::Scalar {
                        data_type: *element_type,
                        nullable: true,
                    }),
                    declaration: Some(OperandDeclaration::Scalar),
                    text_policy: read.text_policies[index],
                    available: true,
                    collection_index: None,
                    collection_origin: Some(
                        read.collection_origin(index)
                            .ok_or(QueryError::Unsupported)?,
                    ),
                    retained_member: true,
                    member: Some(MemberSlot::Value),
                    name: self.copy_name(&read.names[index])?,
                    qualifier: qualifier.map(|name| self.copy_name(name)).transpose()?,
                });
                expansion.push(list);
                list += 1;
            }
        }
        Ok(Relation {
            slots,
            node: RelationNode::Derived {
                read: Box::new(read),
                expansion,
            },
        })
    }
}
