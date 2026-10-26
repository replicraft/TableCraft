//! Metadata proof and exact typed fingerprints; no first-row type inference.
use super::{ExactKeys, check, directory::Directory, fixed::canonical_decimal};
use crate::{
    OutputType, QueryError,
    execution::{Runtime, cell::CellValue},
    plan::{Expr, ProfiledType, Relation},
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionValue, Value},
};
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Null,
    Boolean,
    Byte,
    Int16,
    Int32,
    Int64,
    Currency,
    Decimal,
}

impl Kind {
    fn from_output(output: OutputType) -> Option<Self> {
        match output {
            OutputType::Null => Some(Self::Null),
            OutputType::Scalar { data_type, .. } => match data_type.scalar_type() {
                ScalarType::Boolean => Some(Self::Boolean),
                ScalarType::Byte => Some(Self::Byte),
                ScalarType::Int16 => Some(Self::Int16),
                ScalarType::Int32 => Some(Self::Int32),
                ScalarType::Int64 => Some(Self::Int64),
                ScalarType::Currency => Some(Self::Currency),
                ScalarType::Decimal => Some(Self::Decimal),
                _ => None,
            },
            OutputType::Multivalue { .. } | OutputType::Dynamic => None,
        }
    }
}

impl Runtime<'_, '_> {
    pub(in crate::execution) fn exact_profiled_keys(
        &mut self,
        types: &[ProfiledType],
    ) -> Result<Option<ExactKeys>, QueryError> {
        let declared = self.options.declared_integral;
        let selected = |types: &ProfiledType| {
            if declared {
                types.declared
            } else {
                types.materialized
            }
        };
        for types in types {
            self.tick()?;
            if Kind::from_output(selected(types)).is_none() {
                return Ok(None);
            }
        }
        let mut profile = self.exact_profile(types.len())?;
        for types in types {
            self.tick()?;
            profile.push(Kind::from_output(selected(types)).ok_or(QueryError::Source)?);
        }
        Ok(Some(ExactKeys {
            profile,
            directory: Directory::default(),
            enabled: true,
        }))
    }

    pub(in crate::execution) fn exact_output_keys(
        &mut self,
        types: &[OutputType],
    ) -> Result<Option<ExactKeys>, QueryError> {
        for output in types {
            self.tick()?;
            if Kind::from_output(*output).is_none() {
                return Ok(None);
            }
        }
        let mut profile = self.exact_profile(types.len())?;
        for output in types {
            self.tick()?;
            profile.push(Kind::from_output(*output).ok_or(QueryError::Source)?);
        }
        Ok(Some(ExactKeys {
            profile,
            directory: Directory::default(),
            enabled: true,
        }))
    }

    pub(in crate::execution) fn exact_field_keys(
        &mut self,
        expressions: &[Expr],
        relation: &Relation,
    ) -> Result<Option<ExactKeys>, QueryError> {
        for expression in expressions {
            if self.exact_field_kind(expression, relation)?.is_none() {
                return Ok(None);
            }
        }
        let mut profile = self.exact_profile(expressions.len())?;
        for expression in expressions {
            profile.push(
                self.exact_field_kind(expression, relation)?
                    .ok_or(QueryError::Source)?,
            );
        }
        Ok(Some(ExactKeys {
            profile,
            directory: Directory::default(),
            enabled: true,
        }))
    }

    fn exact_field_kind(
        &mut self,
        expression: &Expr,
        relation: &Relation,
    ) -> Result<Option<Kind>, QueryError> {
        self.tick()?;
        let Expr::Field {
            depth: 0, index, ..
        } = expression
        else {
            return Ok(None);
        };
        let mut available = 0;
        for slot in &relation.slots {
            self.tick()?;
            if !slot.available {
                continue;
            }
            if available == *index {
                let output = if self.options.declared_integral {
                    slot.output_type.declared
                } else {
                    slot.output_type.materialized
                };
                return Ok(Kind::from_output(output));
            }
            available += 1;
        }
        Ok(None)
    }

    fn exact_profile(&mut self, width: usize) -> Result<Vec<Kind>, QueryError> {
        self.budget
            .charge_allocation(width as u64, size_of::<Kind>() as u64)?;
        let mut profile = Vec::new();
        profile
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        Ok(profile)
    }
}

pub(super) fn valid(
    profile: &[Kind],
    row: &[CellValue],
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<bool, QueryError> {
    if profile.len() != row.len() {
        return Ok(false);
    }
    for (kind, cell) in profile.iter().zip(row) {
        check(budget, cancelled)?;
        if matches!(cell.value, ExpressionValue::Value(Value::Null)) {
            continue;
        }
        if !matches!(
            (kind, &cell.value),
            (Kind::Boolean, ExpressionValue::Value(Value::Boolean(_)))
                | (Kind::Byte, ExpressionValue::Value(Value::Byte(_)))
                | (Kind::Int16, ExpressionValue::Value(Value::Int16(_)))
                | (Kind::Int32, ExpressionValue::Value(Value::Int32(_)))
                | (Kind::Int64, ExpressionValue::Value(Value::Int64(_)))
                | (Kind::Currency, ExpressionValue::Value(Value::Currency(_)))
                | (Kind::Decimal, ExpressionValue::Value(Value::Decimal(_)))
        ) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn fingerprint(
    profile: &[Kind],
    row: &[CellValue],
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<Option<u64>, QueryError> {
    if profile.len() != row.len() {
        return Ok(None);
    }
    let mut hash = DefaultHasher::new();
    for (kind, cell) in profile.iter().zip(row) {
        check(budget, cancelled)?;
        if matches!(cell.value, ExpressionValue::Value(Value::Null)) {
            hash.write_u8(0);
            continue;
        }
        hash.write_u8(1);
        match (kind, &cell.value) {
            (Kind::Boolean, ExpressionValue::Value(Value::Boolean(value))) => value.hash(&mut hash),
            (Kind::Byte, ExpressionValue::Value(Value::Byte(value))) => value.hash(&mut hash),
            (Kind::Int16, ExpressionValue::Value(Value::Int16(value))) => value.hash(&mut hash),
            (Kind::Int32, ExpressionValue::Value(Value::Int32(value))) => value.hash(&mut hash),
            (Kind::Int64, ExpressionValue::Value(Value::Int64(value))) => value.hash(&mut hash),
            (Kind::Currency, ExpressionValue::Value(Value::Currency(value))) => {
                value.scaled().hash(&mut hash);
            }
            (Kind::Decimal, ExpressionValue::Value(Value::Decimal(value))) => {
                let (coefficient, scale) = canonical_decimal(*value, budget, cancelled)?;
                coefficient.hash(&mut hash);
                scale.hash(&mut hash);
            }
            _ => return Ok(None),
        }
    }
    Ok(Some(hash.finish()))
}
