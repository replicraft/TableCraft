//! SQL declarations preserve absent policies; table-design defaults do not apply.
use crate::{
    Error,
    controls::check,
    editing::{EditError, platform::table_creation::TableCreationProfile},
};
use tablecraft_ace::{CreateColumn, FirstTableDefinition};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::{DecimalConstraints, NameComparison, ScalarType};
use tablecraft_sql::query::{
    DataType,
    definition::{Constraint, ConstraintKind, CreateTable, TableElement},
};

struct Primary<'a> {
    index: &'a str,
    column: &'a str,
}

fn primary<'a>(constraint: &'a Constraint, inline: Option<&'a str>) -> Result<Primary<'a>, Error> {
    let ConstraintKind::PrimaryKey(fields) = &constraint.kind else {
        return Err(Error::SchemaSqlUnsupported);
    };
    let column = match (inline, fields.as_slice()) {
        (Some(column), []) => column,
        (None, [column]) => column.as_str(),
        _ => return Err(Error::SchemaSqlUnsupported),
    };
    // Unnamed indexes need their actual native naming policy; inventing a name
    // here would change schema metadata and subsequent DROP CONSTRAINT behavior.
    let index = constraint
        .name
        .as_ref()
        .ok_or(Error::SchemaSqlUnsupported)?
        .as_str();
    Ok(Primary { index, column })
}

fn declaration<'a>(name: &'a str, kind: &DataType) -> Result<(CreateColumn<'a>, bool), Error> {
    let (column, generated) = match (kind.name.as_str(), kind.arguments.as_slice()) {
        ("COUNTER" | "AUTOINCREMENT", []) => (CreateColumn::new(name, ScalarType::Int32), true),
        ("BIT" | "BOOLEAN" | "YESNO" | "LOGICAL" | "LOGICAL1", []) => {
            (CreateColumn::new(name, ScalarType::Boolean), false)
        }
        ("BYTE" | "TINYINT" | "INTEGER1", []) => (CreateColumn::new(name, ScalarType::Byte), false),
        ("SHORT" | "SMALLINT" | "INTEGER2", []) => {
            (CreateColumn::new(name, ScalarType::Int16), false)
        }
        ("LONG" | "INTEGER" | "INT" | "LONGINTEGER" | "INTEGER4", []) => {
            (CreateColumn::new(name, ScalarType::Int32), false)
        }
        ("BIGINT" | "LONGLONG", []) => (CreateColumn::new(name, ScalarType::Int64), false),
        ("SINGLE" | "IEEESINGLE" | "REAL" | "FLOAT4", []) => {
            (CreateColumn::new(name, ScalarType::Single), false)
        }
        ("FLOAT" | "DOUBLE" | "IEEEDOUBLE" | "FLOAT8" | "NUMBER", []) => {
            (CreateColumn::new(name, ScalarType::Double), false)
        }
        ("CURRENCY" | "MONEY", []) => (CreateColumn::new(name, ScalarType::Currency), false),
        ("DATETIME" | "DATE" | "TIME", []) => {
            (CreateColumn::new(name, ScalarType::DateTime), false)
        }
        ("GUID" | "UNIQUEIDENTIFIER", []) => (CreateColumn::new(name, ScalarType::Guid), false),
        (
            "CHAR" | "CHARACTER" | "TEXT" | "VARCHAR" | "ALPHANUMERIC" | "STRING" | "NCHAR",
            [units],
        ) => (CreateColumn::short_text(name, *units as usize)?, false),
        ("BINARY" | "VARBINARY", [bytes]) => {
            (CreateColumn::short_binary(name, *bytes as usize)?, false)
        }
        ("DECIMAL" | "NUMERIC" | "DEC", [precision, scale]) => {
            let precision = u8::try_from(*precision).map_err(|_| EditError::InvalidInput)?;
            let scale = u8::try_from(*scale).map_err(|_| EditError::InvalidInput)?;
            let constraints =
                DecimalConstraints::new(precision, scale).map_err(|_| EditError::InvalidInput)?;
            (CreateColumn::decimal(name, constraints), false)
        }
        // Unsized TEXT denotes Long Text in ACE SQL, not a guessed Short Text255.
        // Other omitted parameters require their own established native defaults.
        _ => return Err(Error::SchemaSqlUnsupported),
    };
    Ok((column, generated))
}

pub(super) fn capture(
    table: &CreateTable,
    profile: TableCreationProfile,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<FirstTableDefinition, Error> {
    if table.temporary {
        return Err(Error::SchemaSqlUnsupported);
    }
    let mut selected = None;
    for element in &table.elements {
        check(cancelled)?;
        work.charge(Resource::Steps, 1)?;
        let next = match element {
            TableElement::Column(column) => {
                if column.default_value.is_some() || column.compression {
                    // DEFAULT belongs to OLE DB/ADO, not SQL View. The current
                    // authored native grammar also does not qualify compression.
                    return Err(Error::SchemaSqlUnsupported);
                }
                column
                    .constraint
                    .as_ref()
                    .map(|constraint| primary(constraint, Some(column.name.as_str())))
                    .transpose()?
            }
            TableElement::Constraint(constraint) => Some(primary(constraint, None)?),
        };
        if let Some(next) = next
            && selected.replace(next).is_some()
        {
            return Err(Error::SchemaSqlUnsupported);
        }
    }
    work.charge_allocation(
        table.elements.len() as u64,
        size_of::<CreateColumn<'_>>() as u64,
    )?;
    let mut columns = Vec::new();
    columns.try_reserve_exact(table.elements.len())?;
    let mut generated = false;
    let mut primary_matched = false;
    for element in &table.elements {
        check(cancelled)?;
        let TableElement::Column(field) = element else {
            continue;
        };
        let (mut column, incrementing) = declaration(field.name.as_str(), &field.data_type)?;
        if let Some(primary) = &selected {
            work.charge(
                Resource::Steps,
                field.name.as_str().len().max(primary.column.len()) as u64,
            )?;
            // Native authoring validates names using this same explicit policy.
            if NameComparison::AsciiInsensitive.equals(field.name.as_str(), primary.column) {
                if primary_matched {
                    return Err(Error::SchemaSqlUnsupported);
                }
                primary_matched = true;
                column = column.with_primary_key(primary.index)?;
            }
        }
        if field.not_null {
            column = column.with_required(true);
        }
        if incrementing {
            generated = true;
            column = column.with_incrementing_autonumber()?;
        }
        columns.push(column);
    }
    if selected.is_some() && !primary_matched {
        return Err(Error::SchemaSqlUnsupported);
    }
    let name = table.name.as_str();
    Ok(match (profile, generated) {
        (TableCreationProfile::AddedNative, true) => {
            FirstTableDefinition::capture_native(name, &columns, work, cancelled)?
        }
        (TableCreationProfile::AddedNative, false) => {
            FirstTableDefinition::capture_manual_native(name, &columns, work, cancelled)?
        }
        (_, true) => FirstTableDefinition::capture_authored(name, &columns, work, cancelled)?,
        (_, false) => {
            FirstTableDefinition::capture_manual_authored(name, &columns, work, cancelled)?
        }
    })
}
