//! Metadata inference never evaluates expressions, providers or source records.
mod aggregate;
mod unary;
use super::{Binder, literals};
use crate::{
    OutputType, QueryError,
    plan::{Aggregate, DomainFunction, Expr, Slot},
};
use tablecraft_expressions::functions::conversions::Conversion;
use tablecraft_schema::ScalarType;
use tablecraft_sql::select::{BinaryOperator, Expression};

impl Binder<'_> {
    pub(super) fn type_vector(&mut self, width: usize) -> Result<Vec<OutputType>, QueryError> {
        self.charge(
            width
                .checked_mul(size_of::<OutputType>())
                .ok_or(QueryError::StructureLimit)?,
        )?;
        let mut types = Vec::new();
        types
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        Ok(types)
    }
    pub(super) fn unknown_types(&mut self, width: usize) -> Result<Vec<OutputType>, QueryError> {
        let mut types = self.type_vector(width)?;
        types.resize(width, OutputType::Dynamic);
        Ok(types)
    }
    pub(super) fn result_type(
        &mut self,
        expression: &Expr,
        scopes: &[&[Slot]],
        depth: usize,
    ) -> Result<OutputType, QueryError> {
        self.tick(depth)?;
        Ok(match expression {
            Expr::Field { depth, index, .. } => scopes
                .get(*depth)
                .and_then(|scope| scope.iter().filter(|slot| slot.available).nth(*index))
                .map(|slot| slot.output_type)
                .ok_or(QueryError::Source)?,
            Expr::Parameter(index) => OutputType::Scalar {
                data_type: self
                    .parameters
                    .get(*index)
                    .ok_or(QueryError::Source)?
                    .data_type,
                nullable: true,
            },
            Expr::Literal(Expression::Integer(_)) => OutputType::scalar(ScalarType::Int64, false),
            Expr::Literal(Expression::Text(_)) => OutputType::scalar(ScalarType::Text, false),
            Expr::Literal(Expression::Boolean(_)) => OutputType::scalar(ScalarType::Boolean, false),
            Expr::Literal(Expression::DateLiteral(_)) => {
                OutputType::scalar(ScalarType::DateTime, false)
            }
            Expr::Literal(Expression::GuidLiteral(_)) => {
                OutputType::scalar(ScalarType::Guid, false)
            }
            Expr::Literal(expression @ Expression::Number(_)) => {
                let value = literals::checked(expression)?;
                let scalar = ScalarType::of(&value).ok_or(QueryError::Source)?;
                OutputType::scalar(scalar, false)
            }
            Expr::Unary(operator, argument) => {
                let input = self.result_type(argument, scopes, depth + 1)?;
                unary::result(*operator, input)
            }
            Expr::Convert(conversion, _) => OutputType::scalar(
                match conversion {
                    Conversion::Byte => ScalarType::Byte,
                    Conversion::Integer => ScalarType::Int16,
                    Conversion::Long => ScalarType::Int32,
                    Conversion::LongLong => ScalarType::Int64,
                    Conversion::Boolean => ScalarType::Boolean,
                    Conversion::Single => ScalarType::Single,
                    Conversion::Double => ScalarType::Double,
                    Conversion::Currency => ScalarType::Currency,
                    Conversion::Decimal => ScalarType::Decimal,
                },
                true,
            ),
            Expr::IsNull(..) | Expr::Exists(_) => OutputType::scalar(ScalarType::Boolean, false),
            Expr::Binary(
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual,
                _,
                _,
            )
            | Expr::Like(..)
            | Expr::Between(..)
            | Expr::InList(..)
            | Expr::InQuery(..)
            | Expr::Quantified(..) => OutputType::scalar(ScalarType::Boolean, true),
            Expr::IIf(_, yes, no) => self
                .result_type(yes, scopes, depth + 1)?
                .union(self.result_type(no, scopes, depth + 1)?),
            Expr::Aggregate(kind @ (Aggregate::Sum | Aggregate::Average), Some(argument)) => {
                let input = self.result_type(argument, scopes, depth + 1)?;
                aggregate::numeric(*kind, input)
            }
            Expr::Aggregate(Aggregate::Statistic(_), _)
            | Expr::DomainCall(
                DomainFunction::SampleVariance
                | DomainFunction::PopulationVariance
                | DomainFunction::SampleDeviation
                | DomainFunction::PopulationDeviation,
                _,
            ) => OutputType::scalar(ScalarType::Double, true),
            Expr::DomainCall(DomainFunction::Count, _) => {
                // An unresolved criteria field returns NULL in the domain profile.
                OutputType::scalar(ScalarType::Int64, true)
            }
            Expr::Aggregate(Aggregate::Count, _) => OutputType::scalar(ScalarType::Int64, false),
            Expr::Aggregate(
                Aggregate::Minimum | Aggregate::Maximum | Aggregate::First | Aggregate::Last,
                Some(value),
            ) => self.result_type(value, scopes, depth + 1)?.with_null(),
            Expr::Subquery(read) if read.output_types.len() == 1 => {
                read.output_types[0].with_null()
            }
            // Arithmetic carriers can widen with values. Do not mistake a source
            // declaration or one successful row for a fixed expression type.
            _ => OutputType::Dynamic,
        })
    }
}
