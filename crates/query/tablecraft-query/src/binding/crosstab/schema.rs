//! Named fixed schemas from policy-independent constants; no host code or records.
use crate::{
    QueryError,
    binding::Binder,
    heading_names,
    plan::{Expr, ReadNode, ReadPlan},
    profile::SOURCE_SCHEMA_POLICY,
};
use tablecraft_core::values::Value;
use tablecraft_sql::query::Crosstab;

impl Binder<'_> {
    pub(in crate::binding) fn fixed_crosstab(
        &mut self,
        syntax: &Crosstab,
    ) -> Result<ReadPlan, QueryError> {
        if syntax.headings.is_none() {
            return Err(QueryError::Unsupported);
        }
        let cross = self.crosstab(syntax)?;
        let headings = cross.headings.as_ref().ok_or(QueryError::Unsupported)?;
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
            let name = self.constant_heading_name(heading)?;
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
        Ok(ReadPlan {
            text_policies,
            names,
            node: ReadNode::Crosstab(Box::new(cross)),
            sort: Vec::new(),
            distinct: false,
            top: None,
        })
    }
    fn constant_heading_name(&mut self, expr: &Expr) -> Result<String, QueryError> {
        let value = self.constant_heading(expr, 1)?;
        let integer = match value {
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
