//! Metadata inference never evaluates expressions, providers or source records.
mod aggregate;
mod arithmetic;
mod concatenation;
mod dates;
mod logical;
mod math;
mod null_replacement;
mod selectors;
mod text;
mod unary;
use super::{Binder, literals};
use crate::{
    OutputType, QueryError,
    plan::{Aggregate, DomainFunction, Expr, Slot},
};
use tablecraft_expressions::functions::{
    conversions::Conversion, dates::clock::ClockFunction,
    inspection::types::ValueInspectionFunction,
};
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
            Expr::Parameter(index) => self
                .parameters
                .get(*index)
                .ok_or(QueryError::Source)?
                .data_type
                .output_type(),
            Expr::NumericLiteral(value) => {
                OutputType::scalar(ScalarType::of(value).ok_or(QueryError::Source)?, false)
            }
            Expr::Literal(Expression::Null) => OutputType::Null,
            Expr::Literal(Expression::Integer(_)) => OutputType::scalar(ScalarType::Int64, false),
            Expr::Literal(Expression::Text(_)) => OutputType::scalar(ScalarType::Text, false),
            Expr::Literal(Expression::Boolean(_)) => OutputType::scalar(ScalarType::Boolean, false),
            Expr::DateLiteral(..) | Expr::Literal(Expression::DateLiteral(_)) => {
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
                false,
            ),
            Expr::TextCall(function, arguments) => {
                let mut output = text::result(*function);
                for &index in text::null_inputs(*function, arguments.len()) {
                    let input = arguments.get(index).ok_or(QueryError::Source)?;
                    if self.result_type(input, scopes, depth + 1)? == OutputType::Null {
                        output = OutputType::Null;
                        break;
                    }
                }
                output
            }
            Expr::MathCall(function, arguments) => {
                let input = arguments
                    .first()
                    .map(|argument| self.result_type(argument, scopes, depth + 1))
                    .transpose()?;
                math::result(*function, input)
            }
            Expr::FinancialCall(..) | Expr::CashFlowCall(..) => {
                OutputType::scalar(ScalarType::Double, true)
            }
            Expr::DateCall(_, input) | Expr::DatePart(_, input, _) | Expr::Weekday(input, _) => {
                dates::propagating(
                    self.result_type(input, scopes, depth + 1)?,
                    ScalarType::Int16,
                )
            }
            Expr::DateDiff(_, first, second, _) => {
                let first = self.result_type(first, scopes, depth + 1)?;
                let second = self.result_type(second, scopes, depth + 1)?;
                dates::difference(first, second)
            }
            Expr::DateConvert(function, input) => {
                dates::conversion(*function, self.result_type(input, scopes, depth + 1)?)
            }
            Expr::ConstructorCall(..) => OutputType::scalar(ScalarType::DateTime, false),
            Expr::DateAdd(_, _, date) => {
                dates::addition(self.result_type(date, scopes, depth + 1)?)
            }
            Expr::Clock(function) => OutputType::scalar(
                match function {
                    ClockFunction::Date | ClockFunction::Time | ClockFunction::Now => {
                        ScalarType::DateTime
                    }
                    ClockFunction::Timer => ScalarType::Single,
                },
                false,
            ),
            Expr::Inspect(..) => OutputType::scalar(ScalarType::Boolean, false),
            Expr::ValueInspect(function, _) => OutputType::scalar(
                match function {
                    ValueInspectionFunction::TypeName => ScalarType::Text,
                    ValueInspectionFunction::VarType => ScalarType::Int16,
                    ValueInspectionFunction::IsArray
                    | ValueInspectionFunction::IsEmpty
                    | ValueInspectionFunction::IsError => ScalarType::Boolean,
                },
                false,
            ),
            Expr::GuidFromString(_) => OutputType::scalar(ScalarType::Binary, false),
            Expr::ErrorMessage(_) | Expr::ErrorMessageString(_) => {
                OutputType::scalar(ScalarType::Text, false)
            }
            Expr::StringFromGuid(_) | Expr::CurrentUser => {
                OutputType::scalar(ScalarType::Text, true)
            }
            Expr::CVar(input) => self.result_type(input, scopes, depth + 1)?,
            Expr::IsNull(..)
            | Expr::CollectionEmpty(..)
            | Expr::ParentPresence { .. }
            | Expr::Exists(_) => OutputType::scalar(ScalarType::Boolean, false),
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
            Expr::Binary(BinaryOperator::Concatenate, left, right) => {
                let left = self.result_type(left, scopes, depth + 1)?;
                let right = self.result_type(right, scopes, depth + 1)?;
                concatenation::result(left, right)
            }
            Expr::Binary(
                operator @ (BinaryOperator::And
                | BinaryOperator::Or
                | BinaryOperator::Xor
                | BinaryOperator::Equivalence
                | BinaryOperator::Implication),
                left,
                right,
            ) => {
                let left = self.result_type(left, scopes, depth + 1)?;
                let right = self.result_type(right, scopes, depth + 1)?;
                logical::binary(*operator, left, right)
            }
            Expr::Binary(operator, left, right) => {
                let left = self.result_type(left, scopes, depth + 1)?;
                let right = self.result_type(right, scopes, depth + 1)?;
                arithmetic::result(*operator, left, right)
            }
            Expr::IIf(condition, yes, no) => {
                self.iif_result_type(condition, yes, no, scopes, depth + 1)?
            }
            Expr::ConditionalCall(function, arguments) => {
                self.selection_result_type(*function, arguments, scopes, depth + 1)?
            }
            Expr::Nz(value, replacement) => {
                let input = self.result_type(value, scopes, depth + 1)?;
                if matches!(
                    input,
                    OutputType::Scalar {
                        nullable: false,
                        ..
                    }
                ) {
                    input
                } else {
                    let replacement = replacement
                        .as_ref()
                        .map(|replacement| self.result_type(replacement, scopes, depth + 1))
                        .transpose()?;
                    null_replacement::result(input, replacement)
                }
            }
            Expr::Aggregate(kind @ (Aggregate::Sum | Aggregate::Average), Some(argument)) => {
                self.numeric_aggregate_result_type(*kind, argument, scopes, depth + 1)?
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
            // Unqualified expressions retain visible uncertainty without samples.
            _ => OutputType::Dynamic,
        })
    }
}
