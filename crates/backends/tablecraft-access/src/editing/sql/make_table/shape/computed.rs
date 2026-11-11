//! Fixed computed outputs use established query carriers; native fields keep native sizes.
mod expressions;
use super::{NativeMakeTableError as Error, check, native_column, resolve, table};
use tablecraft_ace::{CreateColumn, InspectError, TableSchema};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_query::MakeTablePlan;
use tablecraft_schema::ScalarType;
use tablecraft_sql::{
    query::{ReadBody, Statement},
    select::{Expression, Projection, Select},
};

/// Validate relation/field syntax before any user-record scan. Computed output
/// carrier checks follow complete query staging; NULL/Dynamic never infer a type.
pub(in crate::editing::sql::make_table) fn preflight(
    select: &Select,
    schema: &TableSchema,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    let table = table(select)?;
    let mut direct_only = true;
    for projection in &select.projection {
        check(work, cancelled)?;
        match projection {
            Projection::Field { field, alias } => {
                let source = resolve(field, table, schema, work, cancelled)?;
                let name = alias.as_ref().unwrap_or(&field.name).as_str();
                let _native_declaration = native_column(name, source)?;
            }
            Projection::Expression {
                expression,
                alias: Some(_),
                ..
            } => {
                expressions::qualify(expression, table, schema, work, cancelled)?;
                direct_only = false;
            }
            _ => return Err(Error::Shape),
        }
    }
    for key in &select.order_by {
        let Expression::Field(reference) = &key.expression else {
            return Err(Error::Shape);
        };
        resolve(reference, table, schema, work, cancelled)?;
    }
    Ok(direct_only)
}

pub(in crate::editing::sql::make_table) fn staged_columns<'a>(
    staged: &'a MakeTablePlan,
    schema: &TableSchema,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CreateColumn<'a>>, Error> {
    let Statement::Read(read) = &staged.retrieval().statement else {
        return Err(Error::Shape);
    };
    let ReadBody::Select(select) = &read.body else {
        return Err(Error::Shape);
    };
    let table = table(select)?;
    if select.projection.len() != staged.columns().len() {
        return Err(Error::Shape);
    }
    work.charge_allocation(
        select.projection.len() as u64,
        size_of::<CreateColumn<'_>>() as u64,
    )
    .map_err(InspectError::from)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(select.projection.len())
        .map_err(|_| InspectError::Allocation)?;
    for (projection, declaration) in select.projection.iter().zip(staged.columns()) {
        check(work, cancelled)?;
        let column = match projection {
            Projection::Field { field, alias } => {
                let source = resolve(field, table, schema, work, cancelled)?;
                let name = alias.as_ref().unwrap_or(&field.name).as_str();
                let column = native_column(name, source)?;
                if column.scalar_type() != declaration.data_type().scalar_type() {
                    return Err(Error::Shape);
                }
                column
            }
            Projection::Expression {
                alias: Some(alias), ..
            } => {
                let scalar = declaration.data_type().scalar_type();
                if !matches!(
                    scalar,
                    ScalarType::Boolean
                        | ScalarType::Byte
                        | ScalarType::Int16
                        | ScalarType::Int32
                        | ScalarType::Int64
                        | ScalarType::Single
                        | ScalarType::Double
                        | ScalarType::Currency
                        | ScalarType::DateTime
                        | ScalarType::Guid
                ) {
                    // Query Unicode-scalar lengths do not establish native UTF16
                    // capacity; unconstrained Decimal has no native p/s declaration.
                    return Err(Error::Shape);
                }
                CreateColumn::new(alias.as_str(), scalar)
            }
            _ => return Err(Error::Shape),
        };
        // Query nullability is never a Required policy. No source PK, counter,
        // default, AZL or description is transferred to this manual column.
        columns.push(column);
    }
    Ok(columns)
}
