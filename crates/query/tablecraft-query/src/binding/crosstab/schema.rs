//! Named fixed schemas from policy-independent constants; no host code or records.
use crate::{
    QueryError,
    binding::Binder,
    heading_names,
    plan::{Expr, ReadNode, ReadPlan},
    profile::SOURCE_SCHEMA_POLICY,
    source::TextPolicy,
};
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_sql::query::{Crosstab, Query};

impl Binder<'_> {
    pub(in crate::binding) fn fixed_crosstab(
        &mut self,
        syntax: &Crosstab,
        definition: &Query,
    ) -> Result<ReadPlan, QueryError> {
        let mut cross = self.crosstab(syntax)?;
        if let Some(index) = self.materialized_index(definition)? {
            let materialized = self.materialized;
            let cached = &materialized[index];
            if cached.result.columns().len() != cached.policies.len()
                || cached.policies.len() > SOURCE_SCHEMA_POLICY.max_columns
            {
                return Err(QueryError::SchemaChanged);
            }
            self.charge(size_of_val(&cross))?;
            cross.materialized = Some(index);
            self.charge(cached.policies.len() * size_of::<TextPolicy>())?;
            let mut policies = Vec::new();
            policies
                .try_reserve_exact(cached.policies.len())
                .map_err(|_| QueryError::Allocation)?;
            policies.extend_from_slice(&cached.policies);
            cross.text_policies = Some(policies);
            let mut names = Vec::new();
            self.charge(size_of_val(cached.result.columns()))?;
            names
                .try_reserve_exact(cached.result.columns().len())
                .map_err(|_| QueryError::Allocation)?;
            for name in cached.result.columns() {
                heading_names::validate(name)?;
                names.push(self.copy_name(name)?);
            }
            let mut text_policies = Vec::new();
            self.charge(cached.policies.len() * size_of::<TextPolicy>())?;
            text_policies
                .try_reserve_exact(cached.policies.len())
                .map_err(|_| QueryError::Allocation)?;
            text_policies.extend_from_slice(&cached.policies);
            if cached.result.output_types().len() != names.len() {
                return Err(QueryError::SchemaChanged);
            }
            let mut output_types = self.type_vector(names.len())?;
            output_types.extend_from_slice(cached.result.output_types());
            return Ok(ReadPlan {
                output_types,
                names,
                text_policies,
                node: ReadNode::Crosstab(Box::new(cross)),
                sort: Vec::new(),
                distinct: false,
                top: None,
            });
        }
        let Some(headings) = cross.headings.as_ref() else {
            return self.request_schema();
        };
        let width = cross
            .read
            .names
            .len()
            .checked_sub(1)
            .ok_or(QueryError::Grouping)?;
        self.charge((width + headings.len()) * size_of::<String>() + size_of_val(&cross))?;
        let mut names = Vec::new();
        names
            .try_reserve_exact(width + headings.len())
            .map_err(|_| QueryError::Allocation)?;
        for name in &cross.read.names[..width] {
            self.tick(1)?;
            heading_names::validate(name)?;
            names.push(self.copy_name(name)?);
        }
        for heading in headings {
            let name = match self.constant_heading_name(heading) {
                Ok(name) => name,
                Err(QueryError::Unsupported) => return self.request_schema(),
                Err(error) => return Err(error),
            };
            for before in &names {
                self.charge(before.len() + name.len())?;
                if SOURCE_SCHEMA_POLICY.names.equals(before, &name) {
                    return Err(QueryError::AmbiguousName);
                }
            }
            names.push(name);
        }
        let mut text_policies = self.policy_vector(names.len())?;
        text_policies.extend_from_slice(
            cross
                .text_policies
                .as_ref()
                .ok_or(QueryError::Unsupported)?,
        );
        let mut output_types = self.type_vector(names.len())?;
        output_types.extend_from_slice(cross.output_types.as_ref().ok_or(QueryError::Source)?);
        Ok(ReadPlan {
            output_types,
            text_policies,
            names,
            node: ReadNode::Crosstab(Box::new(cross)),
            sort: Vec::new(),
            distinct: false,
            top: None,
        })
    }

    fn materialized_index(&mut self, definition: &Query) -> Result<Option<usize>, QueryError> {
        let materialized = self.materialized;
        for (index, cached) in materialized.iter().enumerate() {
            self.tick(1)?;
            let name = self.active_queries.last().ok_or(QueryError::Source)?;
            if SOURCE_SCHEMA_POLICY.names.equals(name, &cached.name) {
                if !std::ptr::eq(definition, cached.definition) {
                    if let Some(budget) = &mut self.budget {
                        budget.charge(Resource::Steps, self.bytes as u64)?;
                    }
                    if definition != cached.definition {
                        return Err(QueryError::SchemaChanged);
                    }
                }
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    fn request_schema(&mut self) -> Result<ReadPlan, QueryError> {
        if self.discover && self.pending_schema.is_none() {
            let length = self.active_queries.last().ok_or(QueryError::Source)?.len();
            self.charge(length)?;
            let mut name = String::new();
            name.try_reserve_exact(length)
                .map_err(|_| QueryError::Allocation)?;
            name.push_str(self.active_queries.last().ok_or(QueryError::Source)?);
            self.pending_schema = Some(name);
        }
        Err(QueryError::Unsupported)
    }

    fn constant_heading_name(&mut self, expr: &Expr) -> Result<String, QueryError> {
        let value = self.constant_heading(expr, 1)?;
        let integer = match value {
            Value::Boolean(value) => return self.copy_name(if value { "True" } else { "False" }),
            Value::Text(text) => {
                heading_names::validate(&text)?;
                return Ok(text);
            }
            Value::Byte(value) => i64::from(value),
            Value::Int16(value) => i64::from(value),
            Value::Int32(value) => i64::from(value),
            Value::Int64(value) => value,
            _ => return Err(QueryError::Unsupported),
        };
        self.charge(heading_names::INTEGER_NAME_BYTES)?;
        heading_names::integer(integer)
    }
}
