//! Evidenced English QBE defaults for plain native Sum/Min/Max fields.
use super::super::build::{name, resolve_symbol};
use super::super::{
    DesignColumn, DesignTableMetadata, DesignTotal, QueryDesignDraft, QueryDesignError,
};
use tablecraft_sql::select::{Expression, FunctionArgument, Name, Projection};

pub(super) fn alias(
    draft: &QueryDesignDraft,
    metadata: &[DesignTableMetadata],
    column: &DesignColumn,
    operand: &Expression,
) -> Result<(Option<Name>, bool), QueryDesignError> {
    if let Some(alias) = &column.alias {
        return Ok((Some(name(alias)?), false));
    }
    if !column.show {
        return Ok((None, false));
    }
    let prefix = match column.total {
        DesignTotal::Sum => "SumOf",
        DesignTotal::Min => "MinOf",
        DesignTotal::Max => "MaxOf",
        _ => return Ok((None, false)),
    };
    let Expression::Call { arguments, .. } = operand else {
        return Ok((None, false));
    };
    let [FunctionArgument::Expression(Expression::Field(field))] = arguments.as_slice() else {
        return Ok((None, false));
    };
    let Some(native) = resolve_symbol(draft, metadata, field)? else {
        return Ok((None, false));
    };
    Ok((Some(name(&format!("{prefix}{}", native.name))?), true))
}

fn output_name(projection: &Projection) -> Option<&str> {
    match projection {
        Projection::Field { field, alias } => Some(alias.as_ref().unwrap_or(&field.name).as_str()),
        Projection::Expression {
            alias: Some(alias), ..
        } => Some(alias.as_str()),
        Projection::Expression {
            expression: Expression::Field(field),
            alias: None,
        } => Some(field.name.as_str()),
        _ => None,
    }
}

pub(super) fn validate_collisions(
    projections: &[Projection],
    generated: &[usize],
) -> Result<(), QueryDesignError> {
    for index in generated {
        let generated_name = projections
            .get(*index)
            .and_then(output_name)
            .ok_or(QueryDesignError::Unsupported)?;
        for (other, projection) in projections.iter().enumerate() {
            if other != *index
                && output_name(projection)
                    .is_some_and(|name| name.eq_ignore_ascii_case(generated_name))
            {
                return Err(QueryDesignError::Unsupported);
            }
        }
    }
    Ok(())
}
