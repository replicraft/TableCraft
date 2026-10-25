//! Bounded same-table calculated dependencies, recomputed without cached inputs.
use super::{CalculationError, syntax};
use tablecraft_ace::{CalculationDeclaration, InspectError, TableFeatures};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::QueryError;
use tablecraft_sql::{
    ParseLimits,
    select::{Expression, FunctionArgument},
};

const MAX_CALCULATED_DEPENDENCIES: usize = 32;

pub(super) fn expand(
    declaration: &CalculationDeclaration,
    limits: ParseLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Expression, Vec<String>), CalculationError> {
    let expression = syntax::parse(declaration.expression()?, limits, budget, cancelled)?;
    let features = declaration.features();
    let root = features
        .schema()
        .columns()
        .iter()
        .position(|field| field.name() == declaration.column().name())
        .ok_or(InspectError::Malformed)?;
    budget
        .charge_allocation(
            MAX_CALCULATED_DEPENDENCIES as u64,
            size_of::<usize>() as u64,
        )
        .map_err(QueryError::from)?;
    let mut active = Vec::new();
    active
        .try_reserve_exact(MAX_CALCULATED_DEPENDENCIES)
        .map_err(|_| QueryError::Allocation)?;
    active.push(root);
    let mut expansion = Expansion {
        features,
        limits,
        budget,
        cancelled,
        active,
        calculated: Vec::new(),
        nodes: 0,
    };
    let expression = expansion.expression(expression, 1)?;
    Ok((expression, expansion.calculated))
}

struct Expansion<'a, 'b> {
    features: &'a TableFeatures,
    limits: ParseLimits,
    budget: &'b mut ResourceBudget,
    cancelled: &'b dyn Fn() -> bool,
    active: Vec<usize>,
    calculated: Vec<String>,
    nodes: usize,
}

impl Expansion<'_, '_> {
    fn tick(&mut self) -> Result<(), CalculationError> {
        if (self.cancelled)() {
            return Err(QueryError::Cancelled.into());
        }
        self.budget
            .charge(Resource::Steps, 1)
            .map_err(QueryError::from)?;
        Ok(())
    }

    fn expression(
        &mut self,
        mut expression: Expression,
        depth: usize,
    ) -> Result<Expression, CalculationError> {
        self.tick()?;
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or(QueryError::StructureLimit)?;
        if self.nodes > self.limits.max_tokens.min(ParseLimits::MAX_TOKENS)
            || depth > self.limits.max_depth.min(ParseLimits::MAX_DEPTH)
        {
            return Err(QueryError::StructureLimit.into());
        }
        match &mut expression {
            Expression::Field(field) if field.qualifier.is_none() => {
                self.budget
                    .charge(
                        Resource::Steps,
                        self.features.schema().columns().len() as u64,
                    )
                    .map_err(QueryError::from)?;
                let position = self
                    .features
                    .schema()
                    .columns()
                    .iter()
                    .position(|column| column.name().eq_ignore_ascii_case(field.name.as_str()))
                    .ok_or(CalculationError::Unsupported)?;
                let column = self
                    .features
                    .columns()
                    .nth(position)
                    .ok_or(InspectError::Malformed)?;
                if !column.calculated() {
                    if column.column().scalar_type().is_none() {
                        return Err(CalculationError::Unsupported);
                    }
                    return Ok(expression);
                }
                // Qualification is identical to explicit stored declarations, but
                // it neither reads wrappers nor turns their caches into operands.
                column.stored_column(self.budget)?;
                if self.active.contains(&position) {
                    return Err(CalculationError::Unsupported);
                }
                if self.active.len() >= MAX_CALCULATED_DEPENDENCIES {
                    return Err(QueryError::StructureLimit.into());
                }
                let name = column.column().name();
                self.budget
                    .charge(Resource::Steps, self.calculated.len() as u64)
                    .map_err(QueryError::from)?;
                if !self.calculated.iter().any(|candidate| candidate == name) {
                    if self.calculated.len() >= MAX_CALCULATED_DEPENDENCIES {
                        return Err(QueryError::StructureLimit.into());
                    }
                    self.budget
                        .charge_allocation(1, size_of::<String>() as u64)
                        .map_err(QueryError::from)?;
                    self.budget
                        .charge_allocation(name.len() as u64, 1)
                        .map_err(QueryError::from)?;
                    self.calculated
                        .try_reserve(1)
                        .map_err(|_| QueryError::Allocation)?;
                    let mut owned = String::new();
                    owned
                        .try_reserve_exact(name.len())
                        .map_err(|_| QueryError::Allocation)?;
                    owned.push_str(name);
                    self.calculated.push(owned);
                }
                let text = column.expression()?.ok_or(InspectError::Malformed)?;
                let nested = syntax::parse(text, self.limits, self.budget, self.cancelled)?;
                self.active.push(position);
                // An alias can add no expression depth. Active-path and node
                // ceilings independently bound chains and shared-graph expansion.
                let expanded = self.expression(nested, depth)?;
                self.active.pop();
                Ok(expanded)
            }
            Expression::Call {
                qualifier,
                name,
                arguments,
            } if syntax::admitted_iif(qualifier, name, arguments) => {
                // Validate and expand every arm before binding, even if the
                // condition could hide it. Runtime eagerness belongs to query.
                for argument in arguments {
                    let FunctionArgument::Expression(operand) = argument else {
                        return Err(CalculationError::Unsupported);
                    };
                    let nested = std::mem::replace(operand, Expression::Null);
                    *operand = self.expression(nested, depth + 1)?;
                }
                Ok(expression)
            }
            Expression::Unary {
                expression: operand,
                ..
            }
            | Expression::IsNull {
                expression: operand,
                ..
            } => {
                let nested = std::mem::replace(operand.as_mut(), Expression::Null);
                **operand = self.expression(nested, depth + 1)?;
                Ok(expression)
            }
            Expression::Binary { left, right, .. } => {
                let first = std::mem::replace(left.as_mut(), Expression::Null);
                **left = self.expression(first, depth + 1)?;
                let second = std::mem::replace(right.as_mut(), Expression::Null);
                **right = self.expression(second, depth + 1)?;
                Ok(expression)
            }
            Expression::Integer(_)
            | Expression::Number(_)
            | Expression::Text(_)
            | Expression::Boolean(_)
            | Expression::Null => Ok(expression),
            _ => Err(CalculationError::Unsupported),
        }
    }
}
