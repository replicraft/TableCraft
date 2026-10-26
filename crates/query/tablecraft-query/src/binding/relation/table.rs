//! Keep physical schema scratch out of recursive named-source dispatch.
use super::Binder;
use crate::{
    OutputType, QueryError, Source,
    binding::source_schema,
    external_sources::SourceNamespace,
    plan::{MemberSlot, ProfiledType, Relation, RelationNode, Slot},
    profile::SOURCE_SCHEMA_POLICY,
    source::TextPolicy,
};
use tablecraft_expressions::numeric::OperandDeclaration;
use tablecraft_schema::{CollectionMember, Column, SchemaName};
use tablecraft_sql::{query::Query, select::Table};

impl Binder<'_> {
    pub(super) fn table_relation(
        &mut self,
        table: &Table,
        depth: usize,
        namespace: SourceNamespace,
    ) -> Result<Relation, QueryError> {
        // A written leaf selector overrides the SELECT's default source
        // namespace. It does not change the read's lexical owner.
        let namespace = self.table_namespace(table.database.as_deref(), namespace, depth)?;
        let provider = self.namespace_source(namespace, depth)?;
        if table.name.as_str().len() > SOURCE_SCHEMA_POLICY.max_name_bytes
            || table
                .alias
                .as_ref()
                .is_some_and(|a| a.as_str().len() > SOURCE_SCHEMA_POLICY.max_name_bytes)
        {
            return Err(QueryError::StructureLimit);
        }
        if self.retrieval
            && let Some(query) = provider.query(table.name.as_str())?
        {
            return self.named_table_relation(table, query, depth, namespace);
        }
        let relation = self.stored_table_relation(table, namespace, provider)?;
        self.finish_relation(relation)
    }

    // Keep owned child-plan moves out of the common table lookup frame.
    #[inline(never)]
    fn named_table_relation(
        &mut self,
        table: &Table,
        query: &Query,
        depth: usize,
        namespace: SourceNamespace,
    ) -> Result<Relation, QueryError> {
        let read = self.named_read(namespace, table.name.as_str(), query, depth + 1)?;
        let qualifier = table.alias.as_ref().unwrap_or(&table.name).as_str();
        self.named_relation(read, qualifier)
    }

    // A large physical-schema frame is needed only at a stored table leaf.
    #[inline(never)]
    fn stored_table_relation(
        &mut self,
        table: &Table,
        namespace: SourceNamespace,
        provider: &dyn Source,
    ) -> Result<Relation, QueryError> {
        let columns = provider.columns(table.name.as_str())?;
        let unavailable = provider.unavailable_columns(table.name.as_str())?;
        source_schema::validate(columns, unavailable)?;
        let policies = provider.text_collations(table.name.as_str())?;
        crate::source::validate_text_collations(columns, policies)?;
        let text_collations = self.copy_text_collations(policies)?;
        let multivalues = provider.multivalue_columns(table.name.as_str())?;
        let members = provider.collection_members(table.name.as_str())?;
        source_schema::validate_multivalues(columns, unavailable, multivalues)?;
        source_schema::validate_members(columns, unavailable, multivalues, members)?;
        let policies = provider.multivalue_text_collations(table.name.as_str())?;
        crate::source::validate_text_collations(multivalues, policies)?;
        let multivalue_text_collations = self.copy_text_collations(policies)?;
        let policies = provider.collection_member_text_collations(table.name.as_str())?;
        crate::source::validate_text_policy_types(
            members.iter().map(|m| m.column().data_type()),
            policies,
        )?;
        let member_text_collations = self.copy_text_collations(policies)?;
        self.charge(
            members
                .iter()
                .map(|m| {
                    2 * m.collection().as_str().len()
                        + 2 * m.column().name().len()
                        + size_of::<CollectionMember>()
                        + size_of::<Slot>()
                        + size_of::<usize>()
                        + SOURCE_SCHEMA_POLICY.max_name_bytes
                })
                .sum(),
        )?;
        self.charge(
            multivalues
                .iter()
                .map(|column| {
                    column.name().len()
                        + size_of::<Column>()
                        + size_of::<Slot>()
                        + size_of::<usize>()
                        + SOURCE_SCHEMA_POLICY.max_name_bytes
                })
                .sum(),
        )?;
        self.charge(
            unavailable
                .iter()
                .map(|name| {
                    name.as_str().len()
                        + SOURCE_SCHEMA_POLICY.max_name_bytes
                        + size_of::<Slot>()
                        + size_of::<SchemaName>()
                })
                .sum(),
        )?;
        self.charge(
            columns
                .iter()
                .map(|c| {
                    c.name().len()
                        + SOURCE_SCHEMA_POLICY.max_name_bytes
                        + std::mem::size_of::<Slot>()
                        + std::mem::size_of::<tablecraft_schema::Column>()
                        + size_of::<usize>()
                })
                .sum(),
        )?;
        Ok(Relation {
            slots: columns
                .iter()
                .enumerate()
                .map(|(index, c)| Slot {
                    output_type: ProfiledType::uniform(OutputType::column(c)),
                    declaration: Some(OperandDeclaration::Scalar),
                    text_policy: text_collations
                        .get(index)
                        .copied()
                        .flatten()
                        .map_or(TextPolicy::Default, TextPolicy::Column),
                    available: true,
                    collection_index: None,
                    member: None,
                    name: c.name().into(),
                    qualifier: Some(table.alias.as_ref().unwrap_or(&table.name).as_str().into()),
                })
                .chain(unavailable.iter().map(|name| {
                    Slot {
                        output_type: ProfiledType::uniform(
                            multivalues
                                .iter()
                                .find(|column| {
                                    SOURCE_SCHEMA_POLICY
                                        .names
                                        .equals(column.name(), name.as_str())
                                })
                                .map_or(OutputType::Dynamic, OutputType::multivalue),
                        ),
                        declaration: None,
                        text_policy: TextPolicy::Default,
                        available: false,
                        collection_index: multivalues.iter().position(|column| {
                            SOURCE_SCHEMA_POLICY
                                .names
                                .equals(column.name(), name.as_str())
                        }),
                        member: None,
                        name: name.as_str().into(),
                        qualifier: Some(
                            table.alias.as_ref().unwrap_or(&table.name).as_str().into(),
                        ),
                    }
                }))
                .chain(multivalues.iter().enumerate().map(|(index, column)| {
                    Slot {
                        output_type: ProfiledType::uniform(OutputType::column(column).with_null()),
                        declaration: Some(OperandDeclaration::Scalar),
                        text_policy: multivalue_text_collations
                            .get(index)
                            .copied()
                            .flatten()
                            .map_or(TextPolicy::Default, TextPolicy::Column),
                        available: true,
                        collection_index: None,
                        member: Some(MemberSlot::Value),
                        name: column.name().into(),
                        qualifier: Some(
                            table.alias.as_ref().unwrap_or(&table.name).as_str().into(),
                        ),
                    }
                }))
                .chain(members.iter().enumerate().map(|(index, member)| {
                    Slot {
                        output_type: ProfiledType::uniform(
                            OutputType::column(member.column()).with_null(),
                        ),
                        declaration: Some(OperandDeclaration::Scalar),
                        text_policy: member_text_collations
                            .get(index)
                            .copied()
                            .flatten()
                            .map_or(TextPolicy::Default, TextPolicy::Column),
                        available: true,
                        collection_index: None,
                        member: Some(MemberSlot::Named(member.column().name().into())),
                        name: member.collection().as_str().into(),
                        qualifier: Some(
                            table.alias.as_ref().unwrap_or(&table.name).as_str().into(),
                        ),
                    }
                }))
                .collect(),
            node: RelationNode::Table {
                namespace,
                name: table.name.as_str().into(),
                columns: columns.to_vec(),
                unavailable: unavailable.to_vec(),
                text_collations,
                projection: (0..columns.len()).collect(),
                multivalues: multivalues.to_vec(),
                multivalue_text_collations,
                member_text_collations,
                members: members.to_vec(),
                expansion: (0..multivalues.len() + members.len()).collect(),
            },
        })
    }
}
