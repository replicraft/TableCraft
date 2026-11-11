//! Closed gallery declarations for adding one field to a saved table.
use super::{CreateColumn, CreateDatabaseError, column::Declaration, defaults::Literal, profile};
use crate::{InspectError, header::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Double, Single, Value},
};
use tablecraft_schema::ScalarType;

/// Validated unnamed type and gallery policy; it carries no source authority.
#[derive(Clone, Copy)]
pub struct RetainedFieldAdditionType {
    declaration: Declaration,
    unicode_compression: bool,
}

impl RetainedFieldAdditionType {
    /// Validate type, exact type-default and field policies without inspecting a name.
    pub fn capture(
        column: CreateColumn<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreateDatabaseError> {
        check(budget, cancelled)?;
        let target = Self {
            declaration: column.declaration(),
            unicode_compression: column.unicode_compression(),
        };
        profile::bind("", target.declaration, 0)?;
        if matches!(target.declaration, Declaration::ShortBinary { .. })
            || column.required() != Some(false)
            || column.allow_zero_length()
                != matches!(target.declaration, Declaration::ShortText { .. }).then_some(true)
            || column.primary_key_name().is_some()
            || column.incrementing_autonumber()
            || column.description().is_some()
        {
            return Err(CreateDatabaseError::Declaration);
        }
        let expected = target.default_value()?;
        if column.default_literal() != expected.as_ref() {
            return Err(CreateDatabaseError::Declaration);
        }
        if let Some(value) = &expected {
            Literal::capture(value, budget, cancelled)?;
        }
        Ok(target)
    }

    /// Exact scalar kind; text capacity and Decimal precision remain native facts.
    pub const fn scalar_type(self) -> ScalarType {
        self.declaration.scalar()
    }

    pub(crate) fn field(
        self,
        fixed_offset: usize,
    ) -> Result<profile::Field<'static>, CreateDatabaseError> {
        let mut field = profile::bind("", self.declaration, fixed_offset)?;
        field.unicode_compression = self.unicode_compression;
        Ok(field)
    }

    pub(crate) fn default_value(self) -> Result<Option<Value>, CreateDatabaseError> {
        Ok(match self.scalar_type() {
            ScalarType::Boolean => Some(Value::Boolean(false)),
            ScalarType::Byte => Some(Value::Byte(0)),
            ScalarType::Int16 => Some(Value::Int16(0)),
            ScalarType::Int32 => Some(Value::Int32(0)),
            ScalarType::Int64 => Some(Value::Int64(0)),
            ScalarType::Currency => Some(Value::Currency(Currency::from_scaled(0))),
            ScalarType::Single => Some(Value::Single(Single::from_bits(0))),
            ScalarType::Double => Some(Value::Double(Double::from_bits(0))),
            ScalarType::Decimal => {
                let Declaration::Decimal(constraints) = self.declaration else {
                    return Err(CreateDatabaseError::Declaration);
                };
                Some(Value::Decimal(
                    Decimal::new(0, constraints.scale())
                        .map_err(|_| CreateDatabaseError::Declaration)?,
                ))
            }
            ScalarType::DateTime | ScalarType::Guid | ScalarType::Text => None,
            _ => return Err(CreateDatabaseError::UnsupportedType),
        })
    }

    pub(crate) fn literal(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<Literal>, CreateDatabaseError> {
        self.default_value()?
            .as_ref()
            .map(|value| Literal::capture(value, budget, cancelled))
            .transpose()
    }

    fn column<'a>(self, name: &'a str, default: Option<&'a Value>) -> CreateColumn<'a> {
        CreateColumn::retained_addition(name, self.declaration, self.unicode_compression, default)
    }
}

/// Owned original request for one gallery field; no fabricated table declaration.
pub struct RetainedFieldAdditionDefinition {
    name: String,
    target: RetainedFieldAdditionType,
    default: Option<Value>,
}

impl RetainedFieldAdditionDefinition {
    /// Validate and retain a single real column, including its exact gallery default.
    pub fn capture(
        column: CreateColumn<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreateDatabaseError> {
        let target = RetainedFieldAdditionType::capture(column, budget, cancelled)?;
        if !crate::identifier::valid_name(column.name(), budget, cancelled)?
            || !column.name().is_ascii()
        {
            return Err(CreateDatabaseError::InvalidName);
        }
        budget.charge_allocation(column.name().len() as u64, 1)?;
        budget.charge(Resource::Steps, column.name().len() as u64)?;
        let mut name = String::new();
        name.try_reserve_exact(column.name().len())
            .map_err(|_| InspectError::Allocation)?;
        name.push_str(column.name());
        Ok(Self {
            name,
            target,
            default: target.default_value()?,
        })
    }

    pub(crate) fn from_name(
        name: String,
        target: RetainedFieldAdditionType,
    ) -> Result<Self, CreateDatabaseError> {
        Ok(Self {
            name,
            target,
            default: target.default_value()?,
        })
    }

    /// Exact requested field spelling.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Name-independent validated declaration for metadata-only availability.
    pub const fn target(&self) -> RetainedFieldAdditionType {
        self.target
    }

    /// Borrow the actual original column and constant default for encoding/replay.
    pub fn column(&self) -> CreateColumn<'_> {
        self.target.column(&self.name, self.default.as_ref())
    }
}
