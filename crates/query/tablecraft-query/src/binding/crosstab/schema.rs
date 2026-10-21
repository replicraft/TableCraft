//! Named schemas from literal headings; never execute code or scan records.
use crate::{
    QueryError,
    binding::Binder,
    heading_names,
    plan::{Expr, ReadNode, ReadPlan},
    profile::SOURCE_SCHEMA_POLICY,
};
use tablecraft_sql::{
    query::Crosstab,
    select::{Expression, UnaryOperator},
};

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
            let name = self.literal_heading_name(heading)?;
            for before in &names {
                self.charge(before.len() + name.len())?;
                if SOURCE_SCHEMA_POLICY.names.equals(before, &name) {
                    return Err(QueryError::AmbiguousName);
                }
            }
            names.push(name);
        }
        Ok(ReadPlan {
            names,
            node: ReadNode::Crosstab(Box::new(cross)),
            sort: Vec::new(),
            distinct: false,
            top: None,
        })
    }
    fn literal_heading_name(&mut self, expr: &Expr) -> Result<String, QueryError> {
        self.tick(1)?;
        if let Expr::Literal(Expression::Text(text)) = expr {
            heading_names::validate(text)?;
            return self.copy_name(text);
        }
        let value = self.heading_integer(expr, 1)?;
        self.charge(heading_names::INTEGER_NAME_BYTES)?;
        heading_names::integer(value)
    }
    fn heading_integer(&mut self, expr: &Expr, depth: usize) -> Result<i64, QueryError> {
        self.tick(depth)?;
        match expr {
            Expr::Literal(Expression::Integer(value)) => Ok(*value),
            Expr::Unary(UnaryOperator::Plus, value) => self.heading_integer(value, depth + 1),
            Expr::Unary(UnaryOperator::Minus, value) => self
                .heading_integer(value, depth + 1)?
                .checked_neg()
                .ok_or(QueryError::Unsupported),
            _ => Err(QueryError::Unsupported),
        }
    }
}
