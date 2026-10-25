//! Reject bare, proven integral equality pairs before allocating a combined row.
use super::Runtime;
use crate::{
    OutputType, QueryError,
    execution::cell::CellValue,
    plan::{Expr, Relation},
    source::TextPolicy,
};
use tablecraft_expressions::comparison::Comparison;
use tablecraft_schema::ScalarType;
use tablecraft_sql::select::BinaryOperator;

pub(super) struct BorrowedEquality {
    first: Field,
    second: Field,
}

enum Field {
    Left(usize),
    Right(usize),
}

impl Field {
    fn value<'a>(
        &self,
        left: &'a [CellValue],
        right: &'a [CellValue],
    ) -> Result<&'a CellValue, QueryError> {
        let value = match self {
            Self::Left(index) => left.get(*index),
            Self::Right(index) => right.get(*index),
        };
        value.ok_or(QueryError::Width)
    }
}

impl Runtime<'_, '_> {
    pub(super) fn borrowed_join_equality(
        &mut self,
        condition: Option<&Expr>,
        left: &Relation,
        right: &Relation,
    ) -> Result<Option<BorrowedEquality>, QueryError> {
        let Some(Expr::Binary(BinaryOperator::Equal, first, second)) = condition else {
            return Ok(None);
        };
        let (
            Expr::Field {
                depth: 0,
                index: first,
                ..
            },
            Expr::Field {
                depth: 0,
                index: second,
                ..
            },
        ) = (first.as_ref(), second.as_ref())
        else {
            return Ok(None);
        };
        self.tick()?;
        let split = left.row_width();
        let (left_index, right_index) = if *first < split && *second >= split {
            (*first, *second - split)
        } else if *second < split && *first >= split {
            (*second, *first - split)
        } else {
            return Ok(None);
        };
        let left_kind = self.integral_join_field(left, left_index)?;
        let right_kind = self.integral_join_field(right, right_index)?;
        if left_kind.is_none() || left_kind != right_kind {
            return Ok(None);
        }

        let (first, second) = if *first < split {
            (Field::Left(left_index), Field::Right(right_index))
        } else {
            (Field::Right(right_index), Field::Left(left_index))
        };
        Ok(Some(BorrowedEquality { first, second }))
    }

    fn integral_join_field(
        &mut self,
        relation: &Relation,
        index: usize,
    ) -> Result<Option<ScalarType>, QueryError> {
        let mut available = 0;
        for slot in &relation.slots {
            self.tick()?;
            if !slot.available {
                continue;
            }
            if available == index {
                let output = if self.options.declared_integral {
                    slot.output_type.declared
                } else {
                    slot.output_type.materialized
                };
                let OutputType::Scalar { data_type, .. } = output else {
                    return Ok(None);
                };
                let scalar = data_type.scalar_type();
                return Ok(matches!(
                    scalar,
                    ScalarType::Boolean
                        | ScalarType::Byte
                        | ScalarType::Int16
                        | ScalarType::Int32
                        | ScalarType::Int64
                )
                .then_some(scalar));
            }
            available += 1;
        }
        Ok(None)
    }

    pub(super) fn borrowed_join_matches(
        &mut self,
        equality: &BorrowedEquality,
        left: &[CellValue],
        right: &[CellValue],
    ) -> Result<bool, QueryError> {
        self.tick()?;
        // Same-carrier integral equality is category-independent. Reuse the
        // scalar predicate kernel, including NULL Unknown, without coercion or
        // representational Value equality. Source rows were validated in full.
        let first = equality.first.value(left, right)?;
        let second = equality.second.value(left, right)?;
        Ok(self
            .compare_expression_predicate_policy(
                Comparison::Equal,
                first,
                second,
                TextPolicy::Default,
            )?
            .is_true())
    }
}
