use super::Binder;
use crate::{OutputType, QueryError, profile::SOURCE_SCHEMA_POLICY};
use tablecraft_core::values::Value;
use tablecraft_expressions::numeric::OperandDeclaration;
use tablecraft_schema::{ScalarType, SchemaName, TypeDescriptor};
use tablecraft_sql::query::DataType;

pub(crate) struct Parameter {
    pub name: SchemaName,
    pub data_type: ParameterType,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParameterType {
    Scalar(TypeDescriptor),
    Value,
}

impl ParameterType {
    pub fn accepts(self, value: &Value) -> bool {
        match self {
            Self::Scalar(data_type) => data_type.validate_value(value).is_ok(),
            Self::Value => true,
        }
    }

    pub fn output_type(self) -> OutputType {
        match self {
            Self::Scalar(data_type) => OutputType::Scalar {
                data_type,
                nullable: true,
            },
            Self::Value => OutputType::Dynamic,
        }
    }

    pub fn declaration(self) -> OperandDeclaration {
        match self {
            Self::Scalar(_) => OperandDeclaration::Scalar,
            Self::Value => OperandDeclaration::Variant,
        }
    }
}

impl Binder<'_> {
    pub(super) fn bind_parameters(
        &mut self,
        declarations: &[tablecraft_sql::query::Parameter],
    ) -> Result<Vec<Parameter>, QueryError> {
        if declarations.len() > crate::profile::MAX_PARAMETERS {
            return Err(QueryError::StructureLimit);
        }
        self.charge(declarations.len() * size_of::<Parameter>())?;
        let mut parameters: Vec<Parameter> = Vec::new();
        parameters
            .try_reserve_exact(declarations.len())
            .map_err(|_| QueryError::Allocation)?;
        for parameter in declarations {
            let text = parameter.name.as_str();
            if text.len() > SOURCE_SCHEMA_POLICY.max_name_bytes {
                return Err(QueryError::StructureLimit);
            }
            self.charge(text.len())?;
            let mut owned = String::new();
            owned
                .try_reserve_exact(text.len())
                .map_err(|_| QueryError::Allocation)?;
            owned.push_str(text);
            let name = SchemaName::new(owned)?;
            SOURCE_SCHEMA_POLICY
                .validate_name(&name)
                .map_err(|_| QueryError::StructureLimit)?;
            if parameters.iter().any(|p| {
                SOURCE_SCHEMA_POLICY
                    .names
                    .equals(p.name.as_str(), name.as_str())
            }) {
                return Err(QueryError::AmbiguousName);
            }
            let data_type = parameter_type(&parameter.data_type)?;
            parameters.push(Parameter { name, data_type });
        }
        Ok(parameters)
    }
}

fn parameter_type(data_type: &DataType) -> Result<ParameterType, QueryError> {
    if data_type.name == "VALUE" {
        if !data_type.arguments.is_empty() {
            return Err(QueryError::Unsupported);
        }
        return Ok(ParameterType::Value);
    }

    let kind = match data_type.name.as_str() {
        "BIT" | "YESNO" | "BOOLEAN" | "LOGICAL" => ScalarType::Boolean,
        "BYTE" | "TINYINT" => ScalarType::Byte,
        "SHORT" | "SMALLINT" => ScalarType::Int16,
        "LONG" | "LONGINTEGER" | "INTEGER" | "INT" => ScalarType::Int32,
        "BIGINT" | "LONGLONG" | "LARGE NUMBER" => ScalarType::Int64,
        "SINGLE" | "REAL" | "IEEESINGLE" => ScalarType::Single,
        "DOUBLE" | "FLOAT" | "IEEEDOUBLE" => ScalarType::Double,
        "CURRENCY" | "MONEY" => ScalarType::Currency,
        "DECIMAL" | "NUMERIC" => ScalarType::Decimal,
        "TEXT" | "CHAR" | "CHARACTER" | "VARCHAR" | "LONGTEXT" | "MEMO" | "LONGCHAR" => {
            ScalarType::Text
        }
        "GUID" | "UNIQUEIDENTIFIER" => ScalarType::Guid,
        "DATETIME" | "DATE" | "TIME" | "TIMESTAMP" => ScalarType::DateTime,
        "DATETIMEEXTENDED" => ScalarType::DateTimeExtended,
        "BINARY" | "IMAGE" | "LONGBINARY" => ScalarType::Binary,
        _ => return Err(QueryError::Unsupported),
    };
    let data_type = match data_type.arguments.as_slice() {
        [] => TypeDescriptor::new(kind),
        [n] if kind == ScalarType::Text && *n > 0 => TypeDescriptor::text(*n as usize)?,
        _ => return Err(QueryError::Unsupported),
    };
    Ok(ParameterType::Scalar(data_type))
}
